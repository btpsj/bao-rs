#![no_std]
#![no_main]

mod pb01;

use embedded_hal::delay::DelayNs;
use embedded_hal::digital::OutputPin;
use panic_halt as _;
use riscv::delay::McycleDelay;

/// The CPU clock boot1 leaves behind; the measured blink period checks this.
const CPU_HZ: u32 = 350_000_000;
const HALF_PERIOD_MS: u32 = 2000;

#[riscv_rt::entry]
fn main() -> ! {
    let mut pin = pb01::PB01::new();
    let mut delay = McycleDelay::new(CPU_HZ);
    loop {
        pin.set_high().unwrap();
        delay.delay_ms(HALF_PERIOD_MS);
        pin.set_low().unwrap();
        delay.delay_ms(HALF_PERIOD_MS);
    }
}
