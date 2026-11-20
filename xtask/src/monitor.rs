//! `cargo xtask flash <bin> --monitor`: a line-based serial terminal on the UART adapter
//! wired to the firmware's UART (PB13/PB14). Ctrl-C ends it.

use std::{
    io::{self, Read, Write},
    thread,
    time::Duration,
};

use eyre::{WrapErr, bail};
use log::info;
use serialport::{SerialPort, SerialPortInfo, SerialPortType};

use crate::flash::{BAUD, BOOT1_USB};

/// The adapter is the one USB serial port that isn't boot1's own.
fn pick(ports: &[SerialPortInfo]) -> eyre::Result<String> {
    let adapters: Vec<&str> = ports
        .iter()
        .filter(|p| matches!(&p.port_type, SerialPortType::UsbPort(usb) if (usb.vid, usb.pid) != BOOT1_USB))
        .map(|p| p.port_name.as_str())
        .collect();
    match adapters[..] {
        [one] => Ok(one.to_owned()),
        [] => bail!("no USB serial adapter found; pass --monitor-port"),
        _ => bail!("several USB serial adapters: {adapters:?}; choose one with --monitor-port"),
    }
}

/// Opens the adapter. Done before flashing, so a missing adapter fails before anything is written.
pub fn open(port: Option<String>) -> eyre::Result<Box<dyn SerialPort>> {
    let name = match port {
        Some(name) => name,
        None => pick(&serialport::available_ports()?)?,
    };
    info!("monitor on {name} at {BAUD} baud");
    serialport::new(&name, BAUD)
        .timeout(Duration::from_millis(50))
        .open()
        .wrap_err_with(|| format!("opening {name}"))
}

/// Prints what the firmware sends and forwards each typed line (ending in `\n`). Runs until
/// Ctrl-C or a port error.
pub fn run(mut port: Box<dyn SerialPort>) -> eyre::Result<()> {
    info!("monitoring; Ctrl-C to quit");
    let mut to_board = port.try_clone().wrap_err("cloning the monitor port")?;
    // Detached: it ends with the process. At end of input it stops and printing carries on.
    thread::spawn(move || {
        for line in io::stdin().lines() {
            let Ok(line) = line else { break };
            if to_board.write_all(format!("{line}\n").as_bytes()).is_err() {
                break;
            }
        }
    });

    let mut stdout = io::stdout();
    let mut buf = [0u8; 1024];
    loop {
        match port.read(&mut buf) {
            Ok(n) => {
                stdout.write_all(&buf[..n])?;
                stdout.flush()?;
            }
            Err(e) if matches!(e.kind(), io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock) => {}
            Err(e) => return Err(e).wrap_err("reading the monitor port"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serialport::UsbPortInfo;

    fn usb(name: &str, vid: u16, pid: u16) -> SerialPortInfo {
        let info = UsbPortInfo { vid, pid, serial_number: None, manufacturer: None, product: None };
        SerialPortInfo { port_name: name.into(), port_type: SerialPortType::UsbPort(info) }
    }

    #[test]
    fn picks_the_only_port_that_is_not_boot1() {
        let boot1 = usb("/dev/ttyACM0", BOOT1_USB.0, BOOT1_USB.1);
        let probe = usb("/dev/ttyACM1", 0x2e8a, 0x000c);
        let onboard = SerialPortInfo { port_name: "/dev/ttyS0".into(), port_type: SerialPortType::Unknown };
        assert_eq!(pick(&[boot1.clone(), onboard.clone(), probe.clone()]).unwrap(), "/dev/ttyACM1");
        assert!(pick(&[boot1.clone(), onboard]).is_err());
        assert!(pick(&[boot1, probe.clone(), usb("/dev/ttyUSB0", 0x0403, 0x6001)]).is_err());
    }
}
