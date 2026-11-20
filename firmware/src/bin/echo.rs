//! UART2 echo demo: type a line, get it back as `<count>: <line>`; PB01 changes level
//! once per line. 115200 8N1 on PB13 (RX) / PB14 (TX).

#![no_std]
#![no_main]

use core::fmt::Write;

use embedded_hal::digital::OutputPin;
use firmware::gpio::Output;
use firmware::peripherals::Peripherals;
use firmware::uart::{Config, Uart};
use panic_halt as _;

#[riscv_rt::entry]
fn main() -> ! {
    let mut p = Peripherals::take().unwrap();
    let mut led = Output::new(p.PB01);
    let mut uart = Uart::new(p.UART2, &mut p.UDMA_CTRL, p.PB13, p.PB14, p.IFRAM0, &p.CGU, Config::default());

    let mut line = [0u8; 64];
    let mut len = 0;
    let mut count = 0u32;
    let mut led_on = false;
    loop {
        let byte = uart.read_byte();
        // Terminals send CR, LF or both for Enter; either ends a line.
        let enter = byte == b'\r' || byte == b'\n';
        if !enter {
            line[len] = byte;
            len += 1;
        }
        // Skipping empty lines also swallows the LF of a CR LF pair.
        if (enter && len > 0) || len == line.len() {
            count += 1;
            write!(uart, "{count}: ").unwrap();
            // The line is raw bytes, not necessarily UTF-8, so it bypasses `write!`.
            let mut rest = &line[..len];
            while !rest.is_empty() {
                let sent = uart.write(rest);
                rest = &rest[sent..];
            }
            uart.write_str("\r\n").unwrap();
            len = 0;

            led_on = !led_on;
            if led_on {
                led.set_high().unwrap();
            } else {
                led.set_low().unwrap();
            }
        }
    }
}
