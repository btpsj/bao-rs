//! Make `memory.x` visible to the linker and use riscv-rt's `link.x`.
//! With riscv-rt's `memory` feature, `link.x` does `INCLUDE memory.x`.

use std::{env, fs, path::PathBuf};

fn main() {
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    fs::copy("memory.x", out.join("memory.x")).unwrap();
    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rerun-if-changed=memory.x");
    println!("cargo:rustc-link-arg-bins=-Tlink.x");
}
