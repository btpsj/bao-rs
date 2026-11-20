#![no_std]
#![no_main]

use core::fmt::Write;
use embedded_hal::delay::DelayNs;
use firmware::peripherals::Peripherals;
use firmware::uart::{Config, Uart};
use panic_halt as _;
use riscv::delay::McycleDelay;

const CPU_HZ: u32 = 350_000_000;
const HALF_PERIOD_MS: u32 = 5000;

#[riscv_rt::entry]
fn main() -> ! {
    let mut p = Peripherals::take().unwrap();
    let mut uart = Uart::new(
        p.UART2,
        &mut p.UDMA_CTRL,
        p.PB13,
        p.PB14,
        p.IFRAM0,
        &p.CGU,
        Config::default(),
    );
    let mut delay = McycleDelay::new(CPU_HZ);
    loop {
        uart.write_str("Clock values\r\n").unwrap();
        write!(uart, "clktop: {:?}\r\n", p.CGU.clktop()).unwrap();
        write!(uart, "sensor valid: {:#x}\r\n", p.CGU.sensor_valid()).unwrap();
        write!(uart, "perclk_hz: {:?}\r\n", p.CGU.perclk_hz()).unwrap();
        write!(uart, "cgufd_per: {:?}\r\n", p.CGU.cgufd_per()).unwrap();
        delay.delay_ms(HALF_PERIOD_MS);
    }
}
