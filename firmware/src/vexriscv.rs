use riscv::{read_only_csr, read_only_csr_field, read_write_csr, read_write_csr_field};

pub mod mim {
    use super::*;

    read_write_csr! {
        /// Machine Interrupt Mask
        Mim: 0xBC0,
        // 0-23 + 30
        mask: 0xffffff | (1 << 30),
    }

    read_write_csr_field! {
        Mim,
        /// TIMER0 peripheral
        timer0: 30,
    }

}

pub mod mip {

    use super::*;

    read_only_csr! {
        /// Machine Interrupt Pending
        Mip: 0xFC0,
        // 0-23 + 30
        mask: 0xffffff | (1 << 30),
    }

    read_only_csr_field! {
        Mip,
        /// TIMER0 peripheral
        timer0: 30,
    }
}
