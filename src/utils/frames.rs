use crate::client::{IncomingMessage, SharedConn};
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
        shared.publish(IncomingMessage::Error {
            raw: s.trim().to_string(),
        });
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
                        shared.publish(IncomingMessage::NvmLine(line.to_string()));
                    }
                }
                return;
            }
        }
    }

    let message = if tag == "reply" {
        IncomingMessage::Response {
            name: name.to_string(),
            id: id.and_then(|value| value.parse().ok()),
            raw: s.trim().to_string(),
        }
    } else {
        IncomingMessage::Event {
            name: name.to_string(),
            id: id.and_then(|value| value.parse().ok()),
            raw: s.trim().to_string(),
        }
    };
    shared.publish(message);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::base64_encode;

    #[test]
    fn publishes_reply_event_and_error_messages() {
        let shared = SharedConn::new("localhost:1".to_string());
        let receiver = shared.subscribe();

        handle_frame(&shared, br#"<reply name="Ping" id="12"/>"#);
        handle_frame(&shared, br#"<event name="VolumeChanged" id="13"/>"#);
        handle_frame(&shared, br#"<error name="Failure">bad command</error>"#);

        match receiver.recv().unwrap() {
            IncomingMessage::Response { name, id, raw } => {
                assert_eq!(name, "Ping");
                assert_eq!(id, Some(12));
                assert_eq!(raw, r#"<reply name="Ping" id="12"/>"#);
            }
            message => panic!("unexpected message: {message:?}"),
        }
        assert!(matches!(
            receiver.recv().unwrap(),
            IncomingMessage::Event { name, id: Some(13), .. } if name == "VolumeChanged"
        ));
        assert!(matches!(
            receiver.recv().unwrap(),
            IncomingMessage::Error { raw } if raw.contains("bad command")
        ));
    }

    #[test]
    fn publishes_complete_nvm_lines_and_keeps_partial_data_buffered() {
        let shared = SharedConn::new("localhost:1".to_string());
        let receiver = shared.subscribe();
        let first = base64_encode(b"volume 15\r\ninput DIG");
        let second = base64_encode(b"ITAL4\r\n");

        handle_frame(
            &shared,
            format!("<event name=\"TunnelFromHost\"><base64>{first}</base64></event>").as_bytes(),
        );
        assert!(matches!(
            receiver.try_recv().unwrap(),
            IncomingMessage::NvmLine(line) if line == "volume 15"
        ));
        assert!(receiver.try_recv().is_err());

        handle_frame(
            &shared,
            format!("<event name=\"TunnelFromHost\"><base64>{second}</base64></event>").as_bytes(),
        );
        assert!(matches!(
            receiver.recv().unwrap(),
            IncomingMessage::NvmLine(line) if line == "input DIGITAL4"
        ));
    }

    #[test]
    fn scan_frame_finds_complete_frame_after_partial_input() {
        assert_eq!(scan_frame(br#"noise<event name="X">"#), None);
        assert_eq!(
            scan_frame(br#"noise<event name="X"></event>tail"#),
            Some((5, 29))
        );
    }
}
