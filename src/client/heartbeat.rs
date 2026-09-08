use super::shared_conn::SharedConn;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

pub fn heartbeat_loop(shared: Arc<SharedConn>, ping_interval_s: u64) {
    loop {
        thread::sleep(Duration::from_secs(ping_interval_s));
        if shared.is_connected() {
            if let Err(e) = shared.send_ping() {
                println!("[naim] Ping send failed: {}", e);
            }
        }
    }
}
