//! `cargo xtask flash`: programs a UF2 through boot1's serial prompt on the board's USB port.
//!
//! Procedure from dabao-sdk's `tools/serial_flash.py`, minus everything that touches
//! bootwait (a one-way counter in the chip): this never changes it.

use std::{
    io::{self, Read, Write},
    thread,
    time::{Duration, Instant},
};

use eyre::{WrapErr, bail, ensure};
use log::info;
use serialport::{ClearBuffer, SerialPort, SerialPortType};

use crate::boot1::{self, Reply};
use crate::{UF2_BLOCK, Uf2Header};

/// boot1's USB serial port.
pub const BOOT1_USB: (u16, u16) = (0x1d50, 0x6196);
pub const BAUD: u32 = 115_200;
/// How long one read blocks; the loops below poll with it.
const READ_TIMEOUT: Duration = Duration::from_millis(50);
/// How long to wait for the board to show up on USB after RESET.
const PORT_TIMEOUT: Duration = Duration::from_secs(60);
const PROMPT_TIMEOUT: Duration = Duration::from_secs(1);
const ACK_TIMEOUT: Duration = Duration::from_secs(3);
const AUDIT_TIMEOUT: Duration = Duration::from_secs(20);
/// Silence that marks the end of a printout.
const QUIET: Duration = Duration::from_millis(400);

/// One read appended to `into`; a timeout is "nothing yet", not an error.
fn read_some(port: &mut impl Read, into: &mut Vec<u8>) -> io::Result<usize> {
    let mut buf = [0u8; 1024];
    match port.read(&mut buf) {
        Ok(n) => {
            into.extend_from_slice(&buf[..n]);
            Ok(n)
        }
        Err(e) if matches!(e.kind(), io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock) => Ok(0),
        Err(e) => Err(e),
    }
}

/// Sends `command`, then reads until `parse` finds its answer in what has arrived.
fn ask<T>(
    port: &mut (impl Read + Write),
    command: &str,
    timeout: Duration,
    parse: impl Fn(&str) -> Option<T>,
) -> eyre::Result<T> {
    port.write_all(command.as_bytes())?;
    port.flush()?;
    let mut received = Vec::new();
    let deadline = Instant::now() + timeout;
    loop {
        read_some(port, &mut received)?;
        let text = String::from_utf8_lossy(&received);
        if let Some(answer) = parse(&text) {
            return Ok(answer);
        }
        if Instant::now() >= deadline {
            let tail: String = text.chars().rev().take(160).collect::<Vec<_>>().into_iter().rev().collect();
            bail!("no answer within {timeout:?}; last received: {tail:?}");
        }
    }
}

/// Discards input until the line has been quiet, so the rest of one printout isn't read as
/// the answer to the next command.
fn drain(port: &mut impl Read) -> eyre::Result<()> {
    let give_up = Instant::now() + AUDIT_TIMEOUT;
    let mut quiet_since = Instant::now();
    while quiet_since.elapsed() < QUIET {
        if read_some(port, &mut Vec::new())? > 0 {
            quiet_since = Instant::now();
        }
        ensure!(Instant::now() < give_up, "the board keeps sending; is this boot1's port?");
    }
    Ok(())
}

/// SHA-512 digests of the boot0 and boot1 partitions. `audit` only reads.
fn audit(port: &mut (impl Read + Write)) -> eyre::Result<(String, String)> {
    let digests = ask(port, "audit\r\n", AUDIT_TIMEOUT, boot1::parse_audit)
        .wrap_err("reading the bootloader digests (`audit`)")?;
    drain(port)?;
    Ok(digests)
}

/// Sends every block, each only after boot1 has acknowledged the one before: boot1 programs
/// flash with its USB handler live, so the wire must be idle while it writes.
fn send_blocks(port: &mut (impl Read + Write), uf2: &[u8]) -> eyre::Result<()> {
    let total = uf2.len() / UF2_BLOCK;
    for (i, block) in uf2.chunks_exact(UF2_BLOCK).enumerate() {
        let h: Uf2Header = bytemuck::pod_read_unaligned(&block[..size_of::<Uf2Header>()]);
        let at = format!("block {}/{total} at {:#010x}", i + 1, h.target);
        match ask(port, &boot1::uf2_command(block), ACK_TIMEOUT, boot1::parse_reply).wrap_err_with(|| at.clone())? {
            Reply::Invalid => bail!("boot1 rejected {at}"),
            Reply::Wrote { len, addr } => ensure!(
                len.is_none_or(|len| len == h.size) && addr.is_none_or(|addr| addr == h.target),
                "{at}, {} bytes: boot1 acknowledged address {addr:#x?}, length {len:?}",
                h.size
            ),
        }
        if i % 50 == 0 || i + 1 == total {
            info!("{at}");
        }
    }
    Ok(())
}

/// The one boot1 port, waiting for it to appear: the board only shows up after RESET.
fn find_port() -> eyre::Result<String> {
    let deadline = Instant::now() + PORT_TIMEOUT;
    let mut told = false;
    loop {
        let mut found: Vec<String> = serialport::available_ports()?
            .into_iter()
            .filter(|p| matches!(&p.port_type, SerialPortType::UsbPort(usb) if (usb.vid, usb.pid) == BOOT1_USB))
            .map(|p| p.port_name)
            .collect();
        match found.len() {
            1 => return Ok(found.remove(0)),
            0 if Instant::now() < deadline => {
                if !told {
                    info!("waiting for boot1 on USB {:04x}:{:04x}: press RESET", BOOT1_USB.0, BOOT1_USB.1);
                    told = true;
                }
                thread::sleep(Duration::from_millis(250));
            }
            0 => bail!("no boot1 serial port appeared within {PORT_TIMEOUT:?}"),
            _ => bail!("several boards in boot1: {found:?}; choose one with --port"),
        }
    }
}

/// Programs `uf2` (already checked by the build) and starts it. `monitor`, the adapter on the
/// firmware's UART, is emptied just before the start so it shows only the new firmware's output.
pub fn flash(uf2: &[u8], port: Option<String>, monitor: Option<&dyn SerialPort>) -> eyre::Result<()> {
    let name = match port {
        Some(name) => name,
        None => find_port()?,
    };
    info!("flashing through {name}");
    let mut port = serialport::new(&name, BAUD)
        .timeout(READ_TIMEOUT)
        .open()
        .wrap_err_with(|| format!("opening {name}"))?;

    // A newline sent just after the port appears can be lost, so ask a few times.
    let mut prompt = Ok(());
    for _ in 0..5 {
        prompt = ask(&mut port, "\r\n", PROMPT_TIMEOUT, |text| boot1::is_prompt(text).then_some(()));
        if prompt.is_ok() {
            break;
        }
    }
    prompt.wrap_err("boot1 is not answering; press RESET and try again")?;
    drain(&mut port)?;

    let before = audit(&mut port)?;
    info!("bootloader digests read; writing {} blocks", uf2.len() / UF2_BLOCK);
    send_blocks(&mut port, uf2)?;
    let after = audit(&mut port)?;
    ensure!(
        before == after,
        "boot0 or boot1 changed during the flash. Do not reset or power-cycle the board: \
         it is still in boot1 now and may not be after a reset"
    );
    info!("bootloader unchanged; starting the firmware");
    if let Some(monitor) = monitor {
        monitor.clear(ClearBuffer::Input)?;
    }
    port.write_all(b"boot\r\n")?;
    port.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    /// Answers each written line with the next scripted reply.
    struct FakePort {
        replies: VecDeque<String>,
        pending: Vec<u8>,
        sent: Vec<u8>,
    }

    impl FakePort {
        fn new(replies: &[&str]) -> Self {
            Self { replies: replies.iter().map(|r| r.to_string()).collect(), pending: Vec::new(), sent: Vec::new() }
        }
    }

    impl Write for FakePort {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.sent.extend_from_slice(buf);
            if buf.ends_with(b"\n") {
                self.pending.extend(self.replies.pop_front().unwrap_or_default().into_bytes());
            }
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl Read for FakePort {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.pending.is_empty() {
                return Err(io::ErrorKind::TimedOut.into());
            }
            let n = self.pending.len().min(buf.len());
            buf[..n].copy_from_slice(&self.pending[..n]);
            self.pending.drain(..n);
            Ok(n)
        }
    }

    /// Three real blocks at 0x60060000, 0x60060100, 0x60060200.
    fn three_blocks() -> Vec<u8> {
        let uf2 = crate::to_uf2(&[0x13; 600]).unwrap();
        assert_eq!(uf2.len(), 3 * UF2_BLOCK);
        uf2
    }

    fn commands(uf2: &[u8], count: usize) -> Vec<u8> {
        uf2.chunks(UF2_BLOCK).take(count).flat_map(|b| boot1::uf2_command(b).into_bytes()).collect()
    }

    #[test]
    fn sends_every_block_once_acknowledged() {
        let uf2 = three_blocks();
        let mut port = FakePort::new(&[
            "Wrote 256 to 0x60060000\r\n",
            "Wrote 256 to 0x60060100\r\n",
            "Wrote 256 to 0x60060200\r\n",
        ]);
        send_blocks(&mut port, &uf2).unwrap();
        assert_eq!(port.sent, commands(&uf2, 3));
    }

    #[test]
    fn stops_when_the_acknowledged_address_differs() {
        let uf2 = three_blocks();
        let mut port = FakePort::new(&["Wrote 256 to 0x60060000\r\n", "Wrote 256 to 0x60020100\r\n"]);
        let err = send_blocks(&mut port, &uf2).unwrap_err();
        assert!(err.to_string().contains("block 2/3"), "{err}");
        // The third block was never sent.
        assert_eq!(port.sent, commands(&uf2, 2));
    }

    #[test]
    fn stops_when_the_acknowledged_length_differs() {
        let uf2 = three_blocks();
        let mut port = FakePort::new(&["Wrote 128 to 0x60060000\r\n"]);
        assert!(send_blocks(&mut port, &uf2).is_err());
        assert_eq!(port.sent, commands(&uf2, 1));
    }

    #[test]
    fn stops_when_boot1_rejects_a_block() {
        let uf2 = three_blocks();
        let mut port = FakePort::new(&["Invalid UF2 block\r\n"]);
        let err = send_blocks(&mut port, &uf2).unwrap_err();
        assert!(err.to_string().contains("rejected block 1/3"), "{err}");
        assert_eq!(port.sent, commands(&uf2, 1));
    }

    #[test]
    fn audit_reads_both_digests_and_discards_the_rest() {
        let (zero, one) = ("0a".repeat(64), "1b".repeat(64));
        let reply = format!("boot0 partition: {zero}\r\nboot1 partition: {one}\r\nWrote nothing, invalid\r\n");
        let mut port = FakePort::new(&[&reply]);
        assert_eq!(audit(&mut port).unwrap(), (zero, one));
        assert!(port.pending.is_empty());
    }
}
