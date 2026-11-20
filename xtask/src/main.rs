//! `cargo xtask build`: turns each firmware binary's ELF into a signed, checked `.uf2`.

mod boot1;

use std::{
    env, fs, io,
    path::{Path, PathBuf},
    process::Command,
};

use bao1x_api::signatures::{
    BAOCHIP_SIG_VERSION, FunctionCode, MAGIC_NUMBER, SIGBLOCK_LEN, SignatureInFlash, UNSIGNED_LEN,
};
use bao1x_api::{BAOCHIP_1X_UF2_FAMILY, BAREMETAL_START};
use eyre::{WrapErr, bail, ensure, eyre};
use log::info;
use xous_tools::sign_image::{Version, bin_to_uf2, load_pem, sign_image};

/// End of the program space; code runs 0x60060300..0x60160000 (FLASH in `firmware/memory.x`).
/// Source: dabao-sdk `bao1x.ld` lines 21-28,
/// https://github.com/ArmstrongSubero/dabao-sdk/blob/fcf8a38873fd742b38da0578a9e86e294cbb0d89/bao1x.ld#L21-L28
const PROGRAM_END: usize = 0x6016_0000;
/// Largest flat `.bin` that fits after the signature block (1,047,296 bytes).
/// A RAM section that became loadable makes objcopy pad the file out to ~16 MiB.
const MAX_BIN: usize = PROGRAM_END - (BAREMETAL_START + SIGBLOCK_LEN);
const TARGET: &str = "riscv32imac-unknown-none-elf";
/// `jal x0, +SIGBLOCK_LEN`: boot1 jumps to the first word of the signature block, which hops to the code.
const JAL_OVER_SIG: u32 = 0x3000_006F;
/// Fixed, never bumped per build: a value above the chip's one-way counter raises it permanently.
const ANTI_ROLLBACK: usize = 1;
const _: () = assert!(
    ANTI_ROLLBACK == 1,
    "ANTI_ROLLBACK is fixed at 1: a higher value permanently raises the chip's counter"
);

// Pod reads use the host's byte order; the image is little-endian (RISC-V).
const _: () = assert!(cfg!(target_endian = "little"));

/// Why a build output failed its format check.
#[derive(Debug, PartialEq, thiserror::Error)]
enum CheckError {
    #[error("bin is empty")]
    BinEmpty,
    #[error("bin is {len} bytes (> {MAX_BIN})")]
    BinTooBig { len: usize },
    #[error("img is {len} bytes, expected {expected} (sig block + bin)")]
    ImgLength { len: usize, expected: usize },
    #[error("img word 0 is {word:#010x}, expected jal x0,+{SIGBLOCK_LEN:#x} ({JAL_OVER_SIG:#010x})")]
    ImgJal { word: u32 },
    #[error("img payload after the sig block differs from the bin")]
    ImgPayload,
    #[error("img magic is {magic:#010x?}, expected \"yumy\" \"Bao3\" ({MAGIC_NUMBER:#010x?})")]
    ImgMagic { magic: [u32; 2] },
    #[error("img version={version:#x}, expected {BAOCHIP_SIG_VERSION:#x}")]
    ImgVersion { version: u32 },
    #[error("img signed_len={signed_len:#x}, expected {expected:#x}")]
    ImgSignedLen { signed_len: u32, expected: u32 },
    #[error("img function_code={function_code}, expected baremetal ({})", FunctionCode::Baremetal as u32)]
    ImgFunctionCode { function_code: u32 },
    #[error("img anti_rollback={anti_rollback}, expected {ANTI_ROLLBACK}")]
    ImgAntiRollback { anti_rollback: u32 },
    #[error("uf2 is {len} bytes, not a multiple of {UF2_BLOCK}")]
    Uf2Length { len: usize },
    #[error("uf2 block {block}: bad magic, flags or payload size")]
    Uf2Header { block: usize },
    #[error("uf2 block {block}: family={family:#010x}, expected {BAOCHIP_1X_UF2_FAMILY:#010x}")]
    Uf2Family { block: usize, family: u32 },
    #[error("uf2 block {block}: numbered {block_no} of {num_blocks}")]
    Uf2Numbering { block: usize, block_no: u32, num_blocks: u32 },
    #[error("uf2 block {block}: target={target:#010x}, not in order inside {BAREMETAL_START:#x}..{PROGRAM_END:#x}")]
    Uf2Target { block: usize, target: u32 },
    #[error("uf2 payloads differ from the img")]
    Uf2Payload,
}

/// The 32-byte header of a 512-byte UF2 block (https://github.com/microsoft/uf2#file-format).
/// xous-tools' `bin_to_uf2` keeps the UF2 constants private, so they are stated here.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uf2Header {
    magic: [u32; 2],
    flags: u32,
    target: u32,
    size: u32,
    block_no: u32,
    num_blocks: u32,
    family: u32,
}
const UF2_BLOCK: usize = 512;
const UF2_PAYLOAD: usize = 256;
const UF2_MAGIC: [u32; 2] = [0x0A32_4655, 0x9E5D_5157];
const UF2_MAGIC_END: u32 = 0x0AB1_6F30;
/// Flag: the `family` field is present.
const UF2_FAMILY_PRESENT: u32 = 0x2000;

/// The flat binary must exist and fit in the program space.
fn check_bin(bin: &[u8]) -> Result<(), CheckError> {
    match bin.len() {
        0 => Err(CheckError::BinEmpty),
        len if len > MAX_BIN => Err(CheckError::BinTooBig { len }),
        _ => Ok(()),
    }
}

/// The header fields boot1 acts on, read back from a checked image.
#[derive(Debug, PartialEq)]
struct SigInfo {
    version: u32,
    function_code: u32,
    anti_rollback: u32,
}


/// The image must be the signature block, starting with the jump over it, followed by `bin` unchanged,
/// and carry the header fields boot1 checks. The header is read as bao1x-api's `SignatureInFlash`, the
/// struct boot1 is built from; reading the bytes back still catches xous-tools writing something else.
fn check_img(bin: &[u8], img: &[u8]) -> Result<SigInfo, CheckError> {
    let expected = SIGBLOCK_LEN + bin.len();
    if img.len() != expected {
        return Err(CheckError::ImgLength { len: img.len(), expected });
    }
    let hdr: SignatureInFlash = bytemuck::pod_read_unaligned(&img[..size_of::<SignatureInFlash>()]);
    if hdr._jal_instruction != JAL_OVER_SIG {
        return Err(CheckError::ImgJal { word: hdr._jal_instruction });
    }
    if img[SIGBLOCK_LEN..] != *bin {
        return Err(CheckError::ImgPayload);
    }

    let sealed = hdr.sealed_data;
    if sealed.magic != MAGIC_NUMBER {
        return Err(CheckError::ImgMagic { magic: sealed.magic });
    }
    if sealed.version != BAOCHIP_SIG_VERSION {
        return Err(CheckError::ImgVersion { version: sealed.version });
    }
    // The signature covers everything from the sealed fields to the end of the image.
    let sealed_len = (img.len() - UNSIGNED_LEN) as u32;
    if sealed.signed_len != sealed_len {
        return Err(CheckError::ImgSignedLen { signed_len: sealed.signed_len, expected: sealed_len });
    }
    if sealed.function_code != FunctionCode::Baremetal as u32 {
        return Err(CheckError::ImgFunctionCode { function_code: sealed.function_code });
    }
    // Still compared at run time: this checks the bytes xous-tools actually wrote.
    if sealed.anti_rollback != ANTI_ROLLBACK as u32 {
        return Err(CheckError::ImgAntiRollback { anti_rollback: sealed.anti_rollback });
    }
    Ok(SigInfo {
        version: sealed.version,
        function_code: sealed.function_code,
        anti_rollback: sealed.anti_rollback,
    })
}

/// The UF2 must carry `img` as consecutive 256-byte payloads addressed from `BAREMETAL_START`,
/// all below `PROGRAM_END`, so boot1 writes it into our slot. Returns the block count.
fn check_uf2(img: &[u8], uf2: &[u8]) -> Result<usize, CheckError> {
    if uf2.len() % UF2_BLOCK != 0 {
        return Err(CheckError::Uf2Length { len: uf2.len() });
    }
    let total = uf2.len() / UF2_BLOCK;
    let mut payload = Vec::with_capacity(total * UF2_PAYLOAD);
    for (block, raw) in uf2.chunks_exact(UF2_BLOCK).enumerate() {
        let h: Uf2Header = bytemuck::pod_read_unaligned(&raw[..size_of::<Uf2Header>()]);
        let magic_end: u32 = bytemuck::pod_read_unaligned(&raw[UF2_BLOCK - 4..]);
        if h.magic != UF2_MAGIC
            || magic_end != UF2_MAGIC_END
            || h.flags & UF2_FAMILY_PRESENT == 0
            || h.size as usize != UF2_PAYLOAD
        {
            return Err(CheckError::Uf2Header { block });
        }
        if h.family != BAOCHIP_1X_UF2_FAMILY {
            return Err(CheckError::Uf2Family { block, family: h.family });
        }
        if h.block_no as usize != block || h.num_blocks as usize != total {
            return Err(CheckError::Uf2Numbering { block, block_no: h.block_no, num_blocks: h.num_blocks });
        }
        let target = h.target as usize;
        if target != BAREMETAL_START + block * UF2_PAYLOAD || target + UF2_PAYLOAD > PROGRAM_END {
            return Err(CheckError::Uf2Target { block, target: h.target });
        }
        let data = size_of::<Uf2Header>();
        payload.extend_from_slice(&raw[data..data + UF2_PAYLOAD]);
    }
    // The last block is zero-padded past the end of the image.
    if payload.len() < img.len()
        || payload[..img.len()] != *img
        || payload[img.len()..].iter().any(|&b| b != 0)
    {
        return Err(CheckError::Uf2Payload);
    }
    Ok(total)
}

/// Paths come from the xtask crate's location, so any working directory works.
fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask is a subdirectory of the workspace")
}

/// The public Xous developer key (`devkey/README.md`); boot1 holds its public half in key slot 3.
fn dev_key() -> PathBuf {
    workspace_root().join("devkey/dev.key")
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

/// The firmware binaries: every `firmware/src/bin/<name>.rs`, by cargo's convention.
fn firmware_bins(root: &Path) -> eyre::Result<Vec<String>> {
    let dir = root.join("firmware/src/bin");
    let mut bins = Vec::new();
    for entry in fs::read_dir(&dir).wrap_err_with(|| format!("reading {}", dir.display()))? {
        let path = entry?.path();
        if path.extension().is_some_and(|ext| ext == "rs") {
            let name = path.file_stem().and_then(|stem| stem.to_str());
            bins.push(name.ok_or_else(|| eyre!("non-UTF-8 file name {}", path.display()))?.to_owned());
        }
    }
    ensure!(!bins.is_empty(), "no binaries in {}", dir.display());
    bins.sort();
    Ok(bins)
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
            "--bins",
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

/// Prepend boot1's signature block to `bin`, signed with `key` as a `baremetal` image.
fn sign(bin: &[u8], key: &Path) -> eyre::Result<Vec<u8>> {
    let key = key.to_str().ok_or_else(|| eyre!("non-UTF-8 key path"))?;
    let pem = load_pem(key).map_err(|e| eyre!("{e}"))?;
    // semver all-zero, min semver unset: boot1 checks neither.
    sign_image(
        bin,
        &pem,
        false,
        &None,
        Some([0u8; 16]),
        true,
        SIGBLOCK_LEN,
        Version::Bao1xV1,
        Some("baremetal"),
        Some(ANTI_ROLLBACK),
    )
    .map_err(|e| eyre!("{e}"))
}

/// Wrap the signed image in UF2 blocks for boot1's USB drive, addressed from `BAREMETAL_START`.
fn to_uf2(img: &[u8]) -> eyre::Result<Vec<u8>> {
    bin_to_uf2(&img.to_vec(), BAOCHIP_1X_UF2_FAMILY, BAREMETAL_START as u32).wrap_err("encoding UF2")
}

fn build() -> eyre::Result<()> {
    let root = workspace_root();
    let target_dir = root.join("target");
    // Cargo's layout: <target-dir>/<triple>/<profile>/<bin>
    let out = target_dir.join(TARGET).join("release");
    let bins = firmware_bins(root)?;

    // A failed build must not leave a previous .uf2 behind to be flashed by mistake,
    // including one from a binary that has since been renamed or removed.
    info!("stage 0: remove old outputs");
    match fs::read_dir(&out) {
        Ok(entries) => {
            for entry in entries {
                let path = entry?.path();
                if path.extension().is_some_and(|ext| ext == "img" || ext == "uf2") {
                    fs::remove_file(&path).wrap_err_with(|| format!("removing {}", path.display()))?;
                    info!("removed {}", path.display());
                }
            }
        }
        // Nothing has been built yet.
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e).wrap_err_with(|| format!("reading {}", out.display())),
    }

    cargo_build(root, &target_dir)?;
    for name in &bins {
        package(&out, name)?;
    }
    Ok(())
}

/// Turns one built ELF, `<out>/<name>`, into `<name>.img` and `<name>.uf2` next to it.
fn package(out: &Path, name: &str) -> eyre::Result<()> {
    info!("packaging {name}");
    let elf = out.join(name);
    let bin_path = out.join(format!("{name}.bin"));
    let img_path = out.join(format!("{name}.img"));
    let uf2_path = out.join(format!("{name}.uf2"));

    rust_objcopy(&elf, &bin_path)?;

    let bin = fs::read(&bin_path).wrap_err_with(|| format!("reading {}", bin_path.display()))?;
    check_bin(&bin)?;
    info!("bin ok: {} bytes", bin.len());

    let key = dev_key();
    info!(
        "stage 3: sign with {} (function_code=baremetal({}), anti_rollback={ANTI_ROLLBACK}, sig block={SIGBLOCK_LEN:#x}, semver=v0.0.0)",
        key.display(),
        FunctionCode::Baremetal as u32
    );
    let img = sign(&bin, &key)?;
    let sig = check_img(&bin, &img)?;
    info!("img ok: {} bytes, {sig:?}", img.len());

    info!("stage 4: img -> UF2");
    let uf2 = to_uf2(&img)?;
    let blocks = check_uf2(&img, &uf2)?;
    info!(
        "uf2 ok: {blocks} blocks {BAREMETAL_START:#x}..{:#x}",
        BAREMETAL_START + blocks * UF2_PAYLOAD
    );

    // Nothing is written before every check has passed; the .uf2 goes last.
    info!("stage 5: write");
    fs::write(&img_path, &img).wrap_err_with(|| format!("writing {}", img_path.display()))?;
    fs::write(&uf2_path, &uf2).wrap_err_with(|| format!("writing {}", uf2_path.display()))?;
    info!("wrote {}", img_path.display());
    info!("wrote {}", uf2_path.display());
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

    /// A hand-built image with the fields boot1 reads, made without xous-tools.
    fn fake_img(bin: &[u8], arb: u32, func: u32) -> Vec<u8> {
        let mut img = vec![0u8; 0x300];
        for (at, value) in [
            (0x00, 0x3000_006F), // jal x0, +0x300
            (0x84, 0x100),       // version
            (0x88, 0x7975_6D79), // "yumy"
            (0x8C, 0x4261_6F33), // "Bao3"
            (0x90, (bin.len() + 0x300 - 0x84) as u32),
            (0x94, func),
            (0x98, arb),
        ] {
            img[at..at + 4].copy_from_slice(&u32::to_le_bytes(value));
        }
        img.extend_from_slice(bin);
        img
    }

    const BAREMETAL_V1_ARB1: SigInfo = SigInfo { version: 0x100, function_code: 6, anti_rollback: 1 };

    #[test]
    fn img_valid_ok() {
        let bin = vec![0x13u8; 100];
        assert_eq!(check_img(&bin, &fake_img(&bin, 1, 6)), Ok(BAREMETAL_V1_ARB1));
    }

    #[test]
    fn img_no_magic_rejected() {
        let bin = vec![0x13u8; 100];
        let mut img = fake_img(&bin, 1, 6);
        img[0x88..0x90].fill(0);
        assert_eq!(check_img(&bin, &img), Err(CheckError::ImgMagic { magic: [0, 0] }));
    }

    #[test]
    fn img_wrong_version_rejected() {
        let bin = vec![0x13u8; 100];
        let mut img = fake_img(&bin, 1, 6);
        img[0x84..0x88].copy_from_slice(&2u32.to_le_bytes()); // Precursor's LoaderPrehash
        assert_eq!(check_img(&bin, &img), Err(CheckError::ImgVersion { version: 2 }));
    }

    #[test]
    fn img_bad_signed_len_rejected() {
        let bin = vec![0x13u8; 100];
        let mut img = fake_img(&bin, 1, 6);
        let good = (100 + 0x300 - 0x84) as u32;
        img[0x90..0x94].copy_from_slice(&(good + 1).to_le_bytes());
        assert_eq!(
            check_img(&bin, &img),
            Err(CheckError::ImgSignedLen { signed_len: good + 1, expected: good })
        );
    }

    #[test]
    fn img_wrong_function_code_rejected() {
        let bin = vec![0x13u8; 100];
        assert_eq!(
            check_img(&bin, &fake_img(&bin, 1, 4)),
            Err(CheckError::ImgFunctionCode { function_code: 4 })
        );
    }

    #[test]
    fn img_anti_rollback_2_rejected() {
        let bin = vec![0x13u8; 100];
        assert_eq!(
            check_img(&bin, &fake_img(&bin, 2, 6)),
            Err(CheckError::ImgAntiRollback { anti_rollback: 2 })
        );
    }

    #[test]
    fn img_wrong_length_rejected() {
        let bin = vec![0x13u8; 100];
        let mut img = fake_img(&bin, 1, 6);
        img.push(0);
        assert_eq!(
            check_img(&bin, &img),
            Err(CheckError::ImgLength { len: 0x300 + 101, expected: 0x300 + 100 })
        );
    }

    #[test]
    fn img_bad_jal_rejected() {
        let bin = vec![0x13u8; 100];
        let mut img = fake_img(&bin, 1, 6);
        img[0..4].fill(0);
        assert_eq!(check_img(&bin, &img), Err(CheckError::ImgJal { word: 0 }));
    }

    #[test]
    fn img_payload_mismatch_rejected() {
        let bin = vec![0x13u8; 100];
        let mut img = fake_img(&bin, 1, 6);
        img[0x300] ^= 0xFF;
        assert_eq!(check_img(&bin, &img), Err(CheckError::ImgPayload));
    }

    /// A 1000-byte bin's fake image (1768 bytes) and its UF2: 7 blocks of 512 bytes.
    fn img_and_uf2() -> (Vec<u8>, Vec<u8>) {
        let img = fake_img(&vec![0x13u8; 1000], 1, 6);
        let uf2 = to_uf2(&img).unwrap();
        (img, uf2)
    }

    #[test]
    fn uf2_valid_ok() {
        let (img, uf2) = img_and_uf2();
        assert_eq!(check_uf2(&img, &uf2), Ok(7));
    }

    #[test]
    fn uf2_block_below_base_rejected() {
        let (img, mut uf2) = img_and_uf2();
        uf2[12..16].copy_from_slice(&0x6005_FF00u32.to_le_bytes());
        assert_eq!(
            check_uf2(&img, &uf2),
            Err(CheckError::Uf2Target { block: 0, target: 0x6005_FF00 })
        );
    }

    #[test]
    fn uf2_wrong_family_rejected() {
        let (img, mut uf2) = img_and_uf2();
        uf2[28..32].fill(0);
        assert_eq!(check_uf2(&img, &uf2), Err(CheckError::Uf2Family { block: 0, family: 0 }));
    }

    #[test]
    fn uf2_truncated_rejected() {
        let (img, mut uf2) = img_and_uf2();
        uf2.pop();
        assert_eq!(check_uf2(&img, &uf2), Err(CheckError::Uf2Length { len: 7 * 512 - 1 }));
    }

    #[test]
    fn uf2_payload_mismatch_rejected() {
        let (img, mut uf2) = img_and_uf2();
        uf2[32] ^= 0xFF;
        assert_eq!(check_uf2(&img, &uf2), Err(CheckError::Uf2Payload));
    }

    #[test]
    fn sign_puts_bin_after_sig_block() {
        let bin = vec![0x13u8; 1000];
        let img = sign(&bin, &dev_key()).unwrap();
        assert_eq!(img.len(), 0x300 + bin.len());
        // jal x0, +0x300: boot1 jumps to 0x60060000, this hops over the signature block.
        assert_eq!(u32::from_le_bytes(img[0..4].try_into().unwrap()), 0x3000_006F);
        assert_eq!(img[0x300..], bin[..]);
    }

    /// Cross-check: the offsets `check_img` reads match what xous-tools actually writes.
    #[test]
    fn sign_passes_check_img() {
        let bin = vec![0x13u8; 1000];
        let img = sign(&bin, &dev_key()).unwrap();
        assert_eq!(check_img(&bin, &img), Ok(BAREMETAL_V1_ARB1));
    }

    /// Verify the way boot1 does: Ed25519ph over SHA-512 of `img[0x84..0x84 + signed_len]`, with the
    /// developer public key boot1 holds in key slot 3. This also proves `devkey/dev.key` is that key.
    #[test]
    fn sign_signature_verifies() {
        use ed25519_dalek::{Signature, VerifyingKey};
        use sha2::{Digest, Sha512};

        let bin = vec![0x13u8; 1000];
        let img = sign(&bin, &dev_key()).unwrap();
        let vk = VerifyingKey::from_bytes(&bao1x_api::pubkeys::developer::PUB).unwrap();

        let signed_len = u32::from_le_bytes(img[0x90..0x94].try_into().unwrap()) as usize;
        let digest = Sha512::new_with_prefix(&img[0x84..0x84 + signed_len]);
        let sig = Signature::from_bytes(img[4..68].try_into().unwrap());
        vk.verify_prehashed(digest, None, &sig).unwrap();
    }
}
