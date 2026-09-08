/// Encode non-text keys using xterm conventions. Printable text and IME commits
/// are delivered separately through GPUI's platform input handler.
pub fn encode_key(
    key: &str,
    control: bool,
    alt: bool,
    shift: bool,
    application_cursor: bool,
) -> Option<Vec<u8>> {
    let modifier = 1 + u8::from(shift) + 2 * u8::from(alt) + 4 * u8::from(control);
    let arrow = match key {
        "up" => Some('A'),
        "down" => Some('B'),
        "right" => Some('C'),
        "left" => Some('D'),
        "home" => Some('H'),
        "end" => Some('F'),
        _ => None,
    };
    if let Some(final_byte) = arrow {
        return Some(
            if modifier > 1 {
                format!("\x1b[1;{modifier}{final_byte}")
            } else if application_cursor {
                format!("\x1bO{final_byte}")
            } else {
                format!("\x1b[{final_byte}")
            }
            .into_bytes(),
        );
    }
    let tilde = match key {
        "insert" => Some(2),
        "delete" => Some(3),
        "pageup" => Some(5),
        "pagedown" => Some(6),
        "f5" => Some(15),
        "f6" => Some(17),
        "f7" => Some(18),
        "f8" => Some(19),
        "f9" => Some(20),
        "f10" => Some(21),
        "f11" => Some(23),
        "f12" => Some(24),
        _ => None,
    };
    if let Some(number) = tilde {
        return Some(
            if modifier > 1 {
                format!("\x1b[{number};{modifier}~")
            } else {
                format!("\x1b[{number}~")
            }
            .into_bytes(),
        );
    }
    if let Some(function) = ["f1", "f2", "f3", "f4"]
        .iter()
        .position(|candidate| *candidate == key)
    {
        let final_byte = (b'P' + function as u8) as char;
        return Some(
            if modifier > 1 {
                format!("\x1b[1;{modifier}{final_byte}")
            } else {
                format!("\x1bO{final_byte}")
            }
            .into_bytes(),
        );
    }
    let data = match key {
        "enter" => vec![b'\r'],
        "backspace" => vec![if control { 8 } else { 127 }],
        "escape" => vec![27],
        "tab" if shift => b"\x1b[Z".to_vec(),
        "tab" => vec![b'\t'],
        _ if control => {
            let byte = match key {
                "space" | "@" | "2" => 0,
                "[" | "3" => 27,
                "\\" | "4" => 28,
                "]" | "5" => 29,
                "^" | "6" => 30,
                "_" | "-" | "7" => 31,
                "?" | "8" => 127,
                _ if key.len() == 1 && key.as_bytes()[0].is_ascii_alphabetic() => {
                    key.as_bytes()[0].to_ascii_uppercase() & 0x1f
                }
                _ => return None,
            };
            vec![byte]
        }
        _ if alt && key.len() == 1 => key.as_bytes().to_vec(),
        _ => return None,
    };
    Some(if alt { [vec![27], data].concat() } else { data })
}

pub fn paste(text: &str, bracketed: bool) -> Vec<u8> {
    // Pasted ESC must not terminate bracketed paste and inject terminal keys.
    let text = text
        .replace('\x1b', "")
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    if bracketed {
        format!("\x1b[200~{text}\x1b[201~").into_bytes()
    } else {
        text.replace('\n', "\r").into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keyboard_protocol() {
        assert_eq!(
            encode_key("up", false, false, false, true).unwrap(),
            b"\x1bOA"
        );
        assert_eq!(
            encode_key("left", true, false, false, false).unwrap(),
            b"\x1b[1;5D"
        );
        assert_eq!(encode_key("c", true, false, false, false).unwrap(), [3]);
        assert_eq!(
            encode_key("f12", false, false, false, false).unwrap(),
            b"\x1b[24~"
        );
        assert!(encode_key("a", false, false, false, false).is_none());
    }
    #[test]
    fn paste_cannot_escape_brackets() {
        assert_eq!(
            paste("a\r\nb\x1b[201~c", true),
            b"\x1b[200~a\nb[201~c\x1b[201~"
        );
    }
}
