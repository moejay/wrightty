use wrightty_protocol::types::{KeyEvent, KeyInput, KeyType, Modifier};

/// Encode a sequence of KeyInput values into bytes to write to a PTY.
pub fn encode_keys(keys: &[KeyInput]) -> Vec<u8> {
    let mut out = Vec::new();
    for key in keys {
        match key {
            KeyInput::Shorthand(s) => encode_shorthand(s, &mut out),
            KeyInput::Structured(event) => encode_key_event(event, &mut out),
        }
    }
    out
}

fn encode_shorthand(s: &str, out: &mut Vec<u8>) {
    // Check for modifier combos like "Ctrl+c"
    if let Some((modifier_str, key_str)) = s.split_once('+') {
        let modifier = match modifier_str {
            "Ctrl" => Some(Modifier::Ctrl),
            "Alt" => Some(Modifier::Alt),
            "Shift" => Some(Modifier::Shift),
            "Meta" => Some(Modifier::Meta),
            _ => None,
        };

        if let Some(m) = modifier {
            // Try to parse the key part as a named key or single char
            if key_str.len() == 1 {
                let event = KeyEvent {
                    key: KeyType::Char,
                    char: Some(key_str.to_string()),
                    n: None,
                    modifiers: vec![m],
                };
                encode_key_event(&event, out);
                return;
            }
            // Named key with modifier
            if let Some(key_type) = parse_named_key(key_str) {
                let event = KeyEvent {
                    key: key_type,
                    char: None,
                    n: None,
                    modifiers: vec![m],
                };
                encode_key_event(&event, out);
                return;
            }
        }
    }

    // Check named keys
    if let Some(key_type) = parse_named_key(s) {
        let event = KeyEvent {
            key: key_type,
            char: None,
            n: None,
            modifiers: vec![],
        };
        encode_key_event(&event, out);
        return;
    }

    // Single character
    if s.len() == 1 {
        out.extend_from_slice(s.as_bytes());
        return;
    }

    // Function keys like "F5"
    if let Some(rest) = s.strip_prefix('F') && let Ok(n) = rest.parse::<u8>() {
        let event = KeyEvent {
            key: KeyType::F,
            char: None,
            n: Some(n),
            modifiers: vec![],
        };
        encode_key_event(&event, out);
        return;
    }

    // Fallback: send as raw text
    out.extend_from_slice(s.as_bytes());
}

fn parse_named_key(s: &str) -> Option<KeyType> {
    match s {
        "Enter" => Some(KeyType::Enter),
        "Tab" => Some(KeyType::Tab),
        "Backspace" => Some(KeyType::Backspace),
        "Delete" => Some(KeyType::Delete),
        "Escape" => Some(KeyType::Escape),
        "ArrowUp" => Some(KeyType::ArrowUp),
        "ArrowDown" => Some(KeyType::ArrowDown),
        "ArrowLeft" => Some(KeyType::ArrowLeft),
        "ArrowRight" => Some(KeyType::ArrowRight),
        "Home" => Some(KeyType::Home),
        "End" => Some(KeyType::End),
        "PageUp" => Some(KeyType::PageUp),
        "PageDown" => Some(KeyType::PageDown),
        "Insert" => Some(KeyType::Insert),
        _ => None,
    }
}

fn encode_key_event(event: &KeyEvent, out: &mut Vec<u8>) {
    let has_ctrl = event.modifiers.iter().any(|m| matches!(m, Modifier::Ctrl));
    let has_alt = event.modifiers.iter().any(|m| matches!(m, Modifier::Alt));

    match &event.key {
        KeyType::Char => {
            if let Some(ref ch) = event.char && let Some(c) = ch.chars().next() {
                if has_ctrl {
                    // Ctrl+letter = letter & 0x1f
                    let ctrl_byte = (c.to_ascii_lowercase() as u8) & 0x1f;
                    if has_alt {
                        out.push(0x1b);
                    }
                    out.push(ctrl_byte);
                } else if has_alt {
                    out.push(0x1b);
                    out.extend_from_slice(ch.as_bytes());
                } else {
                    out.extend_from_slice(ch.as_bytes());
                }
            }
        }
        KeyType::Enter => {
            if has_alt {
                out.push(0x1b);
            }
            out.push(b'\r');
        }
        KeyType::Tab => {
            if has_alt {
                out.push(0x1b);
            }
            out.push(b'\t');
        }
        KeyType::Backspace => {
            if has_alt {
                out.push(0x1b);
            }
            out.push(0x7f);
        }
        KeyType::Escape => {
            out.push(0x1b);
        }
        KeyType::Delete => {
            if has_alt {
                out.push(0x1b);
            }
            out.extend_from_slice(b"\x1b[3~");
        }
        // Arrow keys — normal mode (not application mode for now)
        KeyType::ArrowUp => encode_csi_key(b'A', has_alt, out),
        KeyType::ArrowDown => encode_csi_key(b'B', has_alt, out),
        KeyType::ArrowRight => encode_csi_key(b'C', has_alt, out),
        KeyType::ArrowLeft => encode_csi_key(b'D', has_alt, out),
        KeyType::Home => encode_csi_key(b'H', has_alt, out),
        KeyType::End => encode_csi_key(b'F', has_alt, out),
        KeyType::PageUp => {
            if has_alt {
                out.push(0x1b);
            }
            out.extend_from_slice(b"\x1b[5~");
        }
        KeyType::PageDown => {
            if has_alt {
                out.push(0x1b);
            }
            out.extend_from_slice(b"\x1b[6~");
        }
        KeyType::Insert => {
            if has_alt {
                out.push(0x1b);
            }
            out.extend_from_slice(b"\x1b[2~");
        }
        KeyType::F => {
            let n = event.n.unwrap_or(1);
            let seq = match n {
                1 => b"\x1bOP".as_slice(),
                2 => b"\x1bOQ",
                3 => b"\x1bOR",
                4 => b"\x1bOS",
                5 => b"\x1b[15~",
                6 => b"\x1b[17~",
                7 => b"\x1b[18~",
                8 => b"\x1b[19~",
                9 => b"\x1b[20~",
                10 => b"\x1b[21~",
                11 => b"\x1b[23~",
                12 => b"\x1b[24~",
                _ => return,
            };
            if has_alt {
                out.push(0x1b);
            }
            out.extend_from_slice(seq);
        }
    }
}

fn encode_csi_key(code: u8, alt: bool, out: &mut Vec<u8>) {
    if alt {
        out.push(0x1b);
    }
    out.push(0x1b);
    out.push(b'[');
    out.push(code);
}
