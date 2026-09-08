mod client;
mod config;
mod utils;

use client::{SharedConn, connection_manager, heartbeat_loop};
use config::Config;
use std::io::{self, BufRead};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

fn main() -> anyhow::Result<()> {
    Config::init()?;

    let host = format!(
        "{}:{}",
        Config::global().device_ip,
        Config::global().port.to_string()
    );

    println!("Client Naim — target: {}", host);
    println!(
        "Write NVM commands (without '*' or '\\r'), ex: NVM GETPREAMP, NVM SETRVOL 15, NVM SETINPUT DIGITAL4"
    );
    println!("Write 'quit' to exit.\n");

    let shared = Arc::new(SharedConn::new(host));

    {
        let shared = Arc::clone(&shared);
        thread::spawn(move || connection_manager(shared, Config::global().reconnect));
    }
    {
        let shared = Arc::clone(&shared);
        thread::spawn(move || heartbeat_loop(shared, Config::global().ping_interval));
    }

    // Wait for the first connection to be established before reading stdin.
    thread::sleep(Duration::from_millis(500));

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l.trim().to_string(),
            Err(_) => break,
        };
        if line.is_empty() {
            continue;
        }
        if line.eq_ignore_ascii_case("quit") || line.eq_ignore_ascii_case("exit") {
            break;
        }
        match shared.send_nvm(&line) {
            Ok(()) => println!(">> NVM: {}", line),
            Err(e) => println!("[naim] Error sending NVM command: {}", e),
        }
    }
    Ok(())
}
