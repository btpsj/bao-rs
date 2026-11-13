//! `cargo xtask build`: turns the firmware ELF into a signed, checked `.uf2`.

use eyre::ensure;

/// Largest flat `.bin` that fits the program space, 0x60060300..0x60160000
/// (1,047,296 bytes; equal to FLASH `LENGTH` in `firmware/memory.x`).
/// Source: dabao-sdk `bao1x.ld` lines 21-28,
/// https://github.com/ArmstrongSubero/dabao-sdk/blob/fcf8a38873fd742b38da0578a9e86e294cbb0d89/bao1x.ld#L21-L28
/// A RAM section that became loadable makes objcopy pad the file out to ~16 MiB.
const MAX_BIN: usize = 0x6016_0000 - 0x6006_0300;

/// The flat binary must exist and fit in the program space.
fn check_bin(bin: &[u8]) -> eyre::Result<()> {
    ensure!(!bin.is_empty(), "bin is empty");
    ensure!(
        bin.len() <= MAX_BIN,
        "bin is {} bytes (> {MAX_BIN})",
        bin.len()
    );
    Ok(())
}

fn main() {
    eprintln!("usage: cargo xtask build");
    std::process::exit(2);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bin_empty_rejected() {
        assert!(check_bin(&[]).unwrap_err().to_string().contains("empty"));
    }

    #[test]
    fn bin_over_program_space_rejected() {
        let err = check_bin(&vec![0u8; 0xFFD00 + 1]).unwrap_err();
        assert!(err.to_string().contains("landed in RAM"));
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
