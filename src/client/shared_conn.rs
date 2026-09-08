use crate::config::Config;
use crate::utils::base64_encode;

use std::io::{self, Write};
use std::net::TcpStream;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

pub struct SharedConn {
    host: String,
    pub stream: Mutex<Option<TcpStream>>,
    id_counter: AtomicU32,
    pub nvm_buf: Mutex<String>,
}

impl SharedConn {
    pub fn new(host: String) -> Self {
        SharedConn {
            host,
            stream: Mutex::new(None),
            id_counter: AtomicU32::new(3),
            nvm_buf: Mutex::new(String::new()),
        }
    }

    pub fn is_connected(&self) -> bool {
        self.stream.lock().unwrap().is_some()
    }

    pub fn send_raw(&self, xml: &str) -> io::Result<()> {
        let mut guard = self.stream.lock().unwrap();
        match guard.as_mut() {
            Some(s) => {
                let res = s.write_all(xml.as_bytes());
                if res.is_err() {
                    *guard = None;
                }
                res
            }
            None => Err(io::Error::new(io::ErrorKind::NotConnected, "Not connected")),
        }
    }

    pub fn next_id(&self) -> u32 {
        self.id_counter.fetch_add(1, Ordering::SeqCst)
    }

    pub fn send_nvm(&self, cmd: &str) -> io::Result<()> {
        let id = self.next_id();
        let payload = format!("*{}\r", cmd);
        let b64 = base64_encode(payload.as_bytes());
        let xml = format!(
            "<command name=\"TunnelToHost\" id=\"{}\"><map><item name=\"data\"><base64>{}</base64></item></map></command>",
            id, b64
        );
        self.send_raw(&xml)
    }

    pub fn send_ping(&self) -> io::Result<()> {
        let id = self.next_id();
        let xml = format!("<command name=\"Ping\" id=\"{}\"/>", id);
        self.send_raw(&xml)
    }

    pub fn handshake(&self) -> io::Result<()> {
        self.send_raw(
            "<command name=\"RequestAPIVersion\" id=\"0\"><map><item name=\"module\" string=\"NAIM\"/><item name=\"version\" string=\"1\"/></map></command>",
        )?;
        self.send_raw("<command name=\"GetBridgeCoAppVersions\" id=\"1\"/>")?;
        let hb = format!(
            "<command name=\"SetHeartbeatTimeout\" id=\"2\"><map><item name=\"timeout\" int=\"{}\"/></map></command>",
            Config::global().timeout
        );
        self.send_raw(&hb)?;
        self.id_counter.store(3, Ordering::SeqCst);
        Ok(())
    }

    pub fn get_host(&self) -> &str {
        &self.host
    }
}
