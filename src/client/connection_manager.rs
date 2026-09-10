use std::io;
use std::io::Read;
use std::net::TcpStream;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use super::SharedConn;
use crate::utils::{handle_frame, scan_frame};

// ---------------------------------------------------------------------------
// Loop for reading (runs as long as the connection is alive)
// ---------------------------------------------------------------------------
fn read_loop(shared: &Arc<SharedConn>, mut stream: TcpStream) {
    stream.set_read_timeout(Some(Duration::from_secs(60))).ok();
    let mut buf: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        match stream.read(&mut chunk) {
            Ok(0) => {
                println!("[naim] connection closed by peer (EOF)");
                break;
            }
            Ok(n) => {
                buf.extend_from_slice(&chunk[..n]);
                while let Some((s, e)) = scan_frame(&buf) {
                    let frame: Vec<u8> = buf[s..e].to_vec();
                    buf.drain(0..e);
                    handle_frame(shared, &frame);
                }
            }
            Err(ref e)
                if e.kind() == io::ErrorKind::WouldBlock || e.kind() == io::ErrorKind::TimedOut =>
            {
                continue; // simple wakeup, nothing to do
            }
            Err(e) => {
                println!("[naim] read error: {} — reconnecting...", e);
                break;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Thread manager for (re)connecting indefinitely
// ---------------------------------------------------------------------------

pub fn connection_manager(shared: Arc<SharedConn>, reconnect_delay: u64, timeout: u64) {
    loop {
        let host = shared.get_host();
        match TcpStream::connect(&host) {
            Ok(stream) => {
                stream.set_nodelay(true).ok();
                let reader_stream = match stream.try_clone() {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("[naim] cant clone stream: {}", e);
                        thread::sleep(Duration::from_secs(reconnect_delay));
                        continue;
                    }
                };

                *shared.stream.lock().unwrap() = Some(stream);
                shared.nvm_buf.lock().unwrap().clear();

                if let Err(e) = shared.handshake(timeout) {
                    shared.set_connected(false);
                    println!(
                        "[naim] handshake failed: {} — reconnecting in {}s...",
                        e, reconnect_delay
                    );
                    *shared.stream.lock().unwrap() = None;
                    thread::sleep(Duration::from_secs(reconnect_delay));
                    continue;
                }

                println!(
                    "[naim] connected and session initialized on {}",
                    shared.get_host()
                );
                shared.set_connected(true);
                if let Err(e) = shared.send_nvm("NVM GETPREAMP") {
                    println!("[naim] initial GETPREAMP failed: {}", e);
                }

                // This function blocks as long as the connection is alive.
                read_loop(&shared, reader_stream);

                *shared.stream.lock().unwrap() = None;
                shared.set_connected(false);
                println!(
                    "[naim] connection lost, reconnecting in {}s...",
                    reconnect_delay
                );
            }
            Err(e) => {
                shared.set_connected(false);
                eprintln!(
                    "[naim] cannot connect to {}: {} — reconnecting in {}s",
                    shared.get_host(),
                    e,
                    reconnect_delay
                );
            }
        }
        thread::sleep(Duration::from_secs(reconnect_delay));
    }
}
