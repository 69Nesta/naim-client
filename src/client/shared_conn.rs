use crate::utils::base64_encode;

use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Mutex, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientStatus {
    pub connected: bool,
    pub volume: Option<u8>,
    pub input: Option<String>,
}

#[derive(Debug, Clone)]
pub enum IncomingMessage {
    Response {
        name: String,
        id: Option<u32>,
        raw: String,
    },
    Event {
        name: String,
        id: Option<u32>,
        raw: String,
    },
    Error {
        raw: String,
    },
    NvmLine(String),
    Status(ClientStatus),
}

pub struct SharedConn {
    host: RwLock<String>,
    pub stream: Mutex<Option<TcpStream>>,
    id_counter: AtomicU32,
    pub nvm_buf: Mutex<String>,
    status: Mutex<ClientStatus>,
    subscribers: Mutex<Vec<Sender<IncomingMessage>>>,
}

impl SharedConn {
    pub fn new(host: String) -> Self {
        SharedConn {
            host: RwLock::new(host),
            stream: Mutex::new(None),
            id_counter: AtomicU32::new(3),
            nvm_buf: Mutex::new(String::new()),
            status: Mutex::new(ClientStatus {
                connected: false,
                volume: None,
                input: None,
            }),
            subscribers: Mutex::new(Vec::new()),
        }
    }

    pub fn subscribe(&self) -> Receiver<IncomingMessage> {
        let (sender, receiver) = mpsc::channel();
        self.subscribers.lock().unwrap().push(sender);
        receiver
    }

    pub(crate) fn publish(&self, message: IncomingMessage) {
        let mut subscribers = self.subscribers.lock().unwrap();
        subscribers.retain(|subscriber| subscriber.send(message.clone()).is_ok());
    }

    pub fn is_connected(&self) -> bool {
        self.stream.lock().unwrap().is_some()
    }

    pub fn status(&self) -> ClientStatus {
        self.status.lock().unwrap().clone()
    }

    pub(crate) fn set_connected(&self, connected: bool) {
        let mut status = self.status.lock().unwrap();
        if status.connected == connected {
            return;
        }
        status.connected = connected;
        let snapshot = status.clone();
        drop(status);
        self.publish(IncomingMessage::Status(snapshot));
    }

    pub(crate) fn update_preamp(&self, volume: u8, input: String) {
        let mut status = self.status.lock().unwrap();
        status.volume = Some(volume);
        status.input = Some(input);
        let snapshot = status.clone();
        drop(status);
        self.publish(IncomingMessage::Status(snapshot));
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

    pub fn send_nvm(&self, cmd: &str) -> io::Result<u32> {
        let id = self.next_id();
        let payload = format!("*{}\r", cmd);
        let b64 = base64_encode(payload.as_bytes());
        let xml = format!(
            "<command name=\"TunnelToHost\" id=\"{}\"><map><item name=\"data\"><base64>{}</base64></item></map></command>",
            id, b64
        );
        self.send_raw(&xml).map(|()| id)
    }

    pub fn send_ping(&self) -> io::Result<u32> {
        let id = self.next_id();
        let xml = format!("<command name=\"Ping\" id=\"{}\"/>", id);
        self.send_raw(&xml).map(|()| id)
    }

    pub fn handshake(&self, timeout: u32) -> io::Result<()> {
        self.send_raw(
            "<command name=\"RequestAPIVersion\" id=\"0\"><map><item name=\"module\" string=\"NAIM\"/><item name=\"version\" string=\"1\"/></map></command>",
        )?;
        self.send_raw("<command name=\"GetBridgeCoAppVersions\" id=\"1\"/>")?;
        let hb = format!(
            "<command name=\"SetHeartbeatTimeout\" id=\"2\"><map><item name=\"timeout\" int=\"{}\"/></map></command>",
            timeout
        );
        self.send_raw(&hb)?;
        self.id_counter.store(3, Ordering::SeqCst);
        Ok(())
    }

    pub fn get_host(&self) -> String {
        self.host.read().unwrap().clone()
    }

    pub fn replace_host(&self, host: String) {
        *self.host.write().unwrap() = host;
        self.set_connected(false);
        if let Some(stream) = self.stream.lock().unwrap().take() {
            let _ = stream.shutdown(Shutdown::Both);
        }
    }
}
