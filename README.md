# Exploring bare-metal Rust on the Dabao board

The Baochip-1x on the Dabao board provides a RV32IMAC RISC-V CPU with MMU support. The goal for this is to explore developing firmware and preemptive operating systems with actual hardware.

## Resources

- [Pinout](https://baochip.github.io/dabao/pinout/)

## Build tools

In addition to [`rustup`](https://rustup.rs/) we need `cargo-binutils` for the build pipeline.

```bash
cargo install cargo-binutils  # for rust-objcopy
```

## Flashing

- `cargo xtask build` to build a `.uf2` for every binary in `firmware/src/bin/`
  - e.g. `target/riscv32imac-unknown-none-elf/release/blink.uf2`
- Hold PROG while connecting the Dabao board
- Copy the `.uf2` file to the BAOCHIP volume
- `sync` to flush the write
- Press PROG again to load the firmware
- Measure voltage on PB1 versus GND

## Notes

- After `boot1` stops the USB, it automatically connects to UART2 at 1 Mbaud. This can lead to getting garbled output from the Uart driver. You can either configure the Uart driver and the serial port to read at 1 Mbaud, or introduce a small delay at the start of main to give it time to idle.

```text
// At 115200 baud
���g��ݽ��

// At 1 Mbaud
Stopping USB...
Booting with key 3/3(dev )
Developer key detected, ensuring secrets are erased
```

- Interrupt masks needed for implementing timer interrupts since bao1x-chip doesn't implement a standard RISCV peripheral

| Register | CSR number | Access | Purpose |
|---|---|---|---|
| `mim` | `0xBC0` | read/write | machine interrupt mask (1 = enabled) |
| `mip` | `0xFC0` | read | machine interrupt lines pending |
| `sim` | `0x9C0` | read/write | supervisor interrupt mask |
| `sip` | `0xDC0` | read | supervisor interrupt lines pending |
| `dci` | `0xCC0` | read | data-cache geometry |

## Comparison with similar tools

### `xous-core` bare-metal applications

The [`xous-core`](https://github.com/betrusted-io/xous-core) provides a bare-metal application and provides APIs for various peripherals. This repo focuses on reimplementing much of that support, and also focuses on learning the build process, chip, integrating with Rust embedded ecosystem, and operating system development.

Just as with `xous-core`, uploading an image with the Xous developer key will permanently erase the factory secrets and switch the chip to developer mode. We do reuse parts of the `bao1x-api` to avoid needing to permanently increment the anti-rollback counter.

### `bao1x-bm-hal`

## License

Licensed under either of

- Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or <http://apache.org/licenses/LICENSE-2.0>)
- MIT license
   ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
