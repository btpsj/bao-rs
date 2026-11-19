#![no_std]
#![no_main]

use embedded_hal::delay::DelayNs;
use embedded_hal::digital::OutputPin;
use firmware::gpio::Output;
use firmware::peripherals::Peripherals;
use panic_halt as _;
use riscv::delay::McycleDelay;

/// The CPU clock boot1 leaves behind; the measured blink period checks this.
const CPU_HZ: u32 = 350_000_000;
const HALF_PERIOD_MS: u32 = 2000;

#[riscv_rt::entry]
fn main() -> ! {
    let p = Peripherals::take().unwrap();
    let mut pin = Output::new(p.PB01);
    let mut delay = McycleDelay::new(CPU_HZ);
    loop {
        pin.set_high().unwrap();
        delay.delay_ms(HALF_PERIOD_MS);
        pin.set_low().unwrap();
        delay.delay_ms(HALF_PERIOD_MS);
    }
}
