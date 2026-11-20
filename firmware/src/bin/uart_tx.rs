//! Transmit-only UART2 test, for checking the baud rate with an oscilloscope and a
//! serial adapter. Probe PB14 (TX); PB01 changes level at the start of every burst and
//! can be used as the trigger.

#![no_std]
#![no_main]

use core::fmt::Write;

use embedded_hal::delay::DelayNs;
use firmware::peripherals::Peripherals;
use firmware::uart::{Config, Uart};
use panic_halt as _;
use riscv::delay::McycleDelay;

/// The CPU clock boot1 leaves behind, as measured with `blink`.
const CPU_HZ: u32 = 350_000_000;

/// 'U' is 0x55. Sent LSB first between a low start bit and a high stop bit, every bit
/// differs from the one before, so back-to-back 'U's are a square wave at half the baud rate.
const SQUARE_WAVE: &str = "UUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUU";

#[riscv_rt::entry]
fn main() -> ! {
    let mut p = Peripherals::take().unwrap();
    let mut uart = Uart::new(p.UART2, &mut p.UDMA_CTRL, p.PB13, p.PB14, p.IFRAM0, &p.CGU, Config::default());
    let mut delay = McycleDelay::new(CPU_HZ);

    let mut count = 0u32;
    loop {
        // One string, so it goes out as a single gap-free transfer.
        uart.write_str(SQUARE_WAVE).unwrap();
        // Readable text for the serial adapter.
        write!(uart, "\r\nuart_tx {count}\r\n").unwrap();
        uart.flush();

        count += 1;
        delay.delay_ms(100);
    }
}
