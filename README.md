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

- `cargo xtask build` to build the `firmware.uf2`
  - `target/riscv32imac-unknown-none-elf/release/firmware.uf2`
- Hold PROG while connecting the Dabao board
- Copy the `firmware.uf2` file to the BAOCHIP volume
- `sync` to flush the write
- Press PROG again to load the firmware
- Measure voltage on PB1 versus GND

## versus `xous-core` bare-metal applications

The [`xous-core`](https://github.com/betrusted-io/xous-core) provides a bare-metal application and provides APIs for various peripherals. This repo focuses on reimplementing much of that support, and also focuses on learning the build process, chip, integrating with Rust embedded ecosystem, and operating system development.

Just as with `xous-core`, uploading an image with the Xous developer key will permanently erase the factory secrets and switch the chip to developer mode. We do reuse parts of the `bao1x-api` to avoid needing to permanently increment the anti-rollback counter.

## License

Licensed under either of

- Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or <http://apache.org/licenses/LICENSE-2.0>)
- MIT license
   ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
