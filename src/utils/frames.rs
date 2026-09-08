use crate::client::SharedConn;
use crate::utils::{base64_decode, find_bytes};

pub fn scan_frame(buf: &[u8]) -> Option<(usize, usize)> {
    let candidates: [(&[u8], &[u8]); 3] = [
        (b"<reply", b"</reply>"),
        (b"<event", b"</event>"),
        (b"<error", b"</error>"),
    ];
    let mut best: Option<(usize, usize)> = None;
    for (open, close) in candidates {
        if let Some(s) = find_bytes(buf, open, 0) {
            if let Some(c) = find_bytes(buf, close, s) {
                let e = c + close.len();
                if best.map_or(true, |(bs, _)| s < bs) {
                    best = Some((s, e));
                }
            }
        }
    }
    best
}

fn attr<'a>(frame: &'a str, key: &str) -> Option<&'a str> {
    let pat = format!("{}=\"", key);
    let start = frame.find(&pat)? + pat.len();
    let end = frame[start..].find('"')? + start;
    Some(&frame[start..end])
}

pub fn handle_frame(shared: &SharedConn, raw: &[u8]) {
    let s = String::from_utf8_lossy(raw);

    let tag = if s.starts_with("<reply") {
        "reply"
    } else if s.starts_with("<event") {
        "event"
    } else {
        "error"
    };

    if tag == "error" {
        println!("[naim] ERREUR reçue: {}", s.trim());
        return;
    }

    let name = attr(&s, "name").unwrap_or("?");
    let id = attr(&s, "id");

    // Cas TunnelFromHost / réponses contenant des données NVM encodées en base64
    if let Some(bs) = s.find("<base64>") {
        if let Some(be_rel) = s[bs..].find("</base64>") {
            let b64_raw = &s[bs + 8..bs + be_rel];
            let b64_clean: String = b64_raw.chars().filter(|c| !c.is_whitespace()).collect();
            if let Ok(decoded) = base64_decode(&b64_clean) {
                let text = String::from_utf8_lossy(&decoded).to_string();
                let mut buf = shared.nvm_buf.lock().unwrap();
                buf.push_str(&text);

                while let Some(pos) = buf.find("\r\n") {
                    let line: String = buf.drain(..pos + 2).collect();
                    let line = line.trim_end_matches("\r\n");
                    if !line.is_empty() {
                        println!("<< NVM: {}", line);
                    }
                }
                return;
            }
        }
    }

    match id {
        Some(i) => println!("[naim] {} '{}' (id={}) reçu", tag, name, i),
        None => println!("[naim] {} '{}' reçu", tag, name),
    }
}
