//! `cargo xtask build`: turns the firmware ELF into a signed, checked `.uf2`.

use std::{env, fs, path::Path, process::Command};

use eyre::{WrapErr, bail, ensure};
use log::info;

/// Largest flat `.bin` that fits the program space, 0x60060300..0x60160000
/// (1,047,296 bytes; equal to FLASH `LENGTH` in `firmware/memory.x`).
/// Source: dabao-sdk `bao1x.ld` lines 21-28,
/// https://github.com/ArmstrongSubero/dabao-sdk/blob/fcf8a38873fd742b38da0578a9e86e294cbb0d89/bao1x.ld#L21-L28
/// A RAM section that became loadable makes objcopy pad the file out to ~16 MiB.
const MAX_BIN: usize = 0x6016_0000 - 0x6006_0300;
const TARGET: &str = "riscv32imac-unknown-none-elf";

/// Why a build output failed its format check.
#[derive(Debug, PartialEq, thiserror::Error)]
enum CheckError {
    #[error("bin is empty")]
    BinEmpty,
    #[error("bin is {len} bytes (> {MAX_BIN})")]
    BinTooBig { len: usize },
}

/// The flat binary must exist and fit in the program space.
fn check_bin(bin: &[u8]) -> Result<(), CheckError> {
    match bin.len() {
        0 => Err(CheckError::BinEmpty),
        len if len > MAX_BIN => Err(CheckError::BinTooBig { len }),
        _ => Ok(()),
    }
}

/// Paths come from the xtask crate's location, so any working directory works.
fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask is a subdirectory of the workspace")
}

/// Log the command line, run it, and fail on a non-zero exit.
fn run(cmd: &mut Command) -> eyre::Result<()> {
    info!("running {cmd:?}");
    let status = cmd
        .status()
        .wrap_err_with(|| format!("could not start {:?}", cmd.get_program()))?;
    ensure!(status.success(), "{:?} failed: {status}", cmd.get_program());
    Ok(())
}

fn cargo_build(root: &Path, target_dir: &Path) -> eyre::Result<()> {
    info!("stage 1: build firmware");
    // Allows for cargo +nightly xtask build
    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    run(Command::new(cargo)
        .current_dir(root)
        .args([
            "build",
            "--release",
            "-p",
            "firmware",
            "--target",
            TARGET,
        ])
        // The flag beats CARGO_TARGET_DIR and build.target-dir, so the ELF path below is fixed.
        .arg("--target-dir")
        .arg(target_dir))?;
    Ok(())
}

fn rust_objcopy(elf: &Path, bin_path: &Path) -> eyre::Result<()> {
    info!("stage 2: ELF -> flat binary");
    run(Command::new("rust-objcopy")
        .args(["-O", "binary"])
        .arg(&elf)
        .arg(&bin_path))?;
    Ok(())
}

fn build() -> eyre::Result<()> {
    let root = workspace_root();
    let target_dir = root.join("target");
    // Cargo's layout: <target-dir>/<triple>/<profile>/<bin>
    let out = target_dir.join(TARGET).join("release");
    let elf = out.join("firmware");
    let bin_path = out.join("firmware.bin");

    cargo_build(root, &target_dir)?;
    rust_objcopy(&elf, &bin_path)?;

    let bin = fs::read(&bin_path).wrap_err_with(|| format!("reading {}", bin_path.display()))?;
    check_bin(&bin)?;
    info!("bin ok: {} bytes", bin.len());
    Ok(())
}

fn main() -> eyre::Result<()> {
    // Show info! by default; RUST_LOG still overrides.
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    match env::args().nth(1).as_deref() {
        Some("build") => build(),
        _ => bail!("usage: cargo xtask build"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bin_empty_rejected() {
        assert_eq!(check_bin(&[]), Err(CheckError::BinEmpty));
    }

    #[test]
    fn bin_over_program_space_rejected() {
        assert_eq!(
            check_bin(&vec![0u8; 0xFFD00 + 1]),
            Err(CheckError::BinTooBig { len: 0xFFD00 + 1 })
        );
    }

    #[test]
    fn bin_at_program_space_ok() {
        assert!(check_bin(&vec![0x13u8; 0xFFD00]).is_ok());
    }

    #[test]
    fn bin_small_ok() {
        assert!(check_bin(&vec![0x13u8; 1024]).is_ok());
    }
}
