use core::cell::RefCell;

use critical_section::Mutex;
use firmware::uart::Uart;

pub struct SharedUart(pub(crate) Mutex<RefCell<Option<Uart>>>);

impl SharedUart {
    pub fn init(&self, uart: Uart) {
        critical_section::with(|cs| {
            *self.0.borrow_ref_mut(cs) = Some(uart);
        })
    }
}

pub static UART: SharedUart = SharedUart(Mutex::new(RefCell::new(None)));

#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => {
        use core::fmt::Write;
        use riscv;
        use crate::print::UART;
        riscv::interrupt::free(|| {
            critical_section::with(|cs| {
                let mut uart = UART.0.borrow_ref_mut(cs);
                let uart = uart.as_mut().unwrap();
                let _ = writeln!(uart, $($arg)*);
            } )
        })
    };
}
