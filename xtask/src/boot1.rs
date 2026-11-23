//! The text side of boot1's serial command prompt, as far as flashing needs it: building
//! commands and reading replies. No I/O here.
//!
//! Protocol from dabao-sdk's `tools/serial_flash.py` (which cites boot1's `repl.rs`).

/// The command that programs one 512-byte UF2 block.
pub fn uf2_command(block: &[u8]) -> String {
    format!("uf2 {}\r\n", base64::encode(block))
}

/// boot1's verdict on a `uf2` command.
#[derive(Debug, PartialEq)]
pub enum Reply {
    /// Programmed. Current boot1 reports "Wrote <len> to 0x<addr>"; older versions report
    /// only the address, or nothing.
    Wrote { len: Option<u32>, addr: Option<u32> },
    /// boot1 refused the block.
    Invalid,
}

/// Looks for boot1's verdict in everything received since the command was sent.
/// `None` means it hasn't arrived yet: keep reading.
pub fn parse_reply(received: &str) -> Option<Reply> {
    // Only complete lines, so a half-received "Wrote 25" isn't read as a short write.
    let complete = &received[..received.rfind('\n')? + 1];
    for line in complete.lines() {
        // boot1 may echo the command; its base64 must not be searched for keywords.
        if line.trim_start().starts_with("uf2 ") {
            continue;
        }
        let lower = line.to_ascii_lowercase();
        if lower.contains("invalid") {
            return Some(Reply::Invalid);
        }
        if let Some(at) = lower.find("wrote") {
            let mut words = line[at + "wrote".len()..].split_whitespace();
            return Some(match (words.next(), words.next(), words.next()) {
                (Some(len), Some("to"), Some(addr)) => Reply::Wrote {
                    len: len.parse().ok(),
                    addr: hex(addr),
                },
                (Some(addr), _, _) => Reply::Wrote {
                    len: None,
                    addr: hex(addr),
                },
                _ => Reply::Wrote {
                    len: None,
                    addr: None,
                },
            });
        }
    }
    None
}

fn hex(word: &str) -> Option<u32> {
    u32::from_str_radix(word.trim_start_matches("0x"), 16).ok()
}

/// Whether `received` looks like boot1's answer to an empty line (its command list).
pub fn is_prompt(received: &str) -> bool {
    ["Commands include", "uf2", "bootwait"]
        .iter()
        .any(|word| received.contains(word))
}

/// The SHA-512 digests of the boot0 and boot1 partitions from an `audit` printout,
/// once both have arrived. Comparing them before and after a flash shows whether the
/// bootloader was disturbed.
pub fn parse_audit(received: &str) -> Option<(String, String)> {
    Some((
        digest_after(received, "boot0 partition:")?,
        digest_after(received, "boot1 partition:")?,
    ))
}

fn digest_after(text: &str, label: &str) -> Option<String> {
    let rest = text[text.find(label)? + label.len()..].trim_start();
    let digest: String = rest.chars().take_while(char::is_ascii_hexdigit).collect();
    // SHA-512 as hex; fewer digits means the line is still arriving.
    (digest.len() == 128).then(|| digest.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uf2_command_is_one_base64_line() {
        assert_eq!(uf2_command(b"abc"), "uf2 YWJj\r\n");
        // A real block: 512 bytes is 684 base64 characters.
        assert_eq!(uf2_command(&[0u8; 512]).len(), "uf2 ".len() + 684 + 2);
    }

    #[test]
    fn reply_with_length_and_address() {
        assert_eq!(
            parse_reply("Wrote 256 to 0x60060000\r\n"),
            Some(Reply::Wrote {
                len: Some(256),
                addr: Some(0x6006_0000)
            })
        );
    }

    #[test]
    fn reply_from_older_boot1() {
        assert_eq!(
            parse_reply("Wrote 60060100\r\n"),
            Some(Reply::Wrote {
                len: None,
                addr: Some(0x6006_0100)
            })
        );
        assert_eq!(
            parse_reply("Wrote\r\n"),
            Some(Reply::Wrote {
                len: None,
                addr: None
            })
        );
    }

    #[test]
    fn reply_invalid() {
        assert_eq!(parse_reply("Invalid UF2 block\r\n"), Some(Reply::Invalid));
    }

    #[test]
    fn reply_waits_for_a_complete_line() {
        assert_eq!(parse_reply(""), None);
        assert_eq!(parse_reply("Wrote 25"), None);
        assert_eq!(parse_reply("\r\n"), None);
    }

    #[test]
    fn reply_ignores_the_echoed_command() {
        // Base64 that happens to spell the keywords.
        let echo = "uf2 AAAAwroteAAAAinvalidAAAA\r\n";
        assert_eq!(parse_reply(echo), None);
        assert_eq!(
            parse_reply(&format!("{echo}Wrote 256 to 0x60060000\r\n")),
            Some(Reply::Wrote {
                len: Some(256),
                addr: Some(0x6006_0000)
            })
        );
    }

    #[test]
    fn prompt_detection() {
        assert!(is_prompt("Commands include: reset, boot, uf2, bootwait"));
        assert!(!is_prompt("1: hello\r\n"));
    }

    #[test]
    fn audit_digests() {
        let (zero, one) = ("0A".repeat(64), "1b".repeat(64));
        let text = format!("boot0 partition: {zero}\r\nboot1 partition: {one}\r\n");
        assert_eq!(
            parse_audit(&text),
            Some((zero.to_ascii_lowercase(), one.clone()))
        );
        // boot1's digest still arriving.
        assert_eq!(parse_audit(&text[..text.len() - 20]), None);
        assert_eq!(parse_audit("boot0 partition: abcd\r\n"), None);
    }
}
