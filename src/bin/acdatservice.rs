//! `acdatservice` command-line interface.
//!
//! Currently exposes `generate`, which turns DAT files into per-landblock
//! ACLB bundle files. The byte-level format lives in [`acdatservice::bundle`]
//! and is documented in `docs/bundle-format.md`.

use std::collections::BTreeMap;
use std::error::Error;
use std::fs::{self, File};
use std::path::PathBuf;

use acdatservice::bundle::{self, Bundle, BundleEntry, BundleKind};
use acprotocol::dat::reader::{
    sync_dat_file_reader::SyncDatFileReader, sync_file_reader::SyncFileRangeReader,
    types::dat_database::DatDatabase,
};
use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(name = "acdatservice", version, about = "Asheron's Call DAT tooling")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Generate per-landblock ACLB bundle files from a DAT.
    Generate(GenerateArgs),
}

#[derive(Args)]
struct GenerateArgs {
    /// Bundle to generate.
    #[arg(value_enum)]
    file_type: FileType,

    /// Output directory (created if missing). One file per landblock.
    #[arg(short, long, default_value = "./generated")]
    output: PathBuf,

    /// Generate only this landblock: hex (0xB34A) or decimal (45898).
    #[arg(long)]
    block: Option<String>,

    /// Path to client_cell_1.dat. Defaults to $DATS_DIR/client_cell_1.dat.
    #[arg(long)]
    dat: Option<PathBuf>,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
enum FileType {
    /// Terrain (…FFFF) plus LandblockInfo (…FFFE), when present.
    Landblock,
    /// Every EnvCell (…01xx–…FExx) owned by the block.
    Envcell,
}

impl FileType {
    fn kind(self) -> BundleKind {
        match self {
            FileType::Landblock => BundleKind::Landblock,
            FileType::Envcell => BundleKind::Envcell,
        }
    }

    fn name(self) -> &'static str {
        match self {
            FileType::Landblock => "landblock",
            FileType::Envcell => "envcell",
        }
    }

    /// Subdirectory that mirrors the R2 prefix used by the bundle route.
    fn collection(self) -> &'static str {
        match self {
            FileType::Landblock => "landblocks",
            FileType::Envcell => "envcells",
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    match Cli::parse().command {
        Command::Generate(args) => generate(args),
    }
}

fn generate(args: GenerateArgs) -> Result<(), Box<dyn Error>> {
    let dat_path = args.dat.clone().unwrap_or_else(default_cell_dat_path);
    if !dat_path.is_file() {
        return Err(format!(
            "cell DAT not found: {}\nSet DATS_DIR or pass --dat.",
            dat_path.display()
        )
        .into());
    }

    let block_filter = match args.block.as_deref() {
        Some(text) => Some(parse_landblock(text)?),
        None => None,
    };

    let mut dat_file = File::open(&dat_path)?;
    let database = DatDatabase::read(&mut dat_file)?;
    let block_size = database.header.block_size as usize;
    let files = database.list_files(true)?;
    let mut range_reader = SyncFileRangeReader::new(dat_file);

    // Matching files grouped by landblock, as (id, offset, size).
    let mut grouped: BTreeMap<u16, Vec<(u32, u32, u32)>> = BTreeMap::new();
    for file in files {
        if file.file_size == 0 || !matches_file_type(args.file_type, file.object_id) {
            continue;
        }
        let block = (file.object_id >> 16) as u16;
        if block_filter.is_some_and(|wanted| wanted != block) {
            continue;
        }
        grouped
            .entry(block)
            .or_default()
            .push((file.object_id, file.file_offset, file.file_size));
    }

    let output_dir = args.output.join(args.file_type.collection());
    fs::create_dir_all(&output_dir)?;

    let mut written = 0usize;
    for (block, mut files) in grouped {
        files.sort_by_key(|(id, _, _)| *id);

        let mut entries = Vec::with_capacity(files.len());
        for (id, offset, size) in files {
            let mut file_reader = SyncDatFileReader::new(size as usize, block_size)?;
            let data = file_reader.read_file(&mut range_reader, offset)?;
            entries.push(BundleEntry { id, data });
        }

        let bundle = Bundle {
            kind: args.file_type.kind(),
            landblock: block,
            entries,
        };
        let path = output_dir.join(output_file_name(block));
        fs::write(&path, bundle::encode(&bundle))?;
        written += 1;
    }

    if written == 0 {
        eprintln!("no matching files found for {}", args.file_type.name());
    } else {
        println!(
            "wrote {} {} file(s) to {}",
            written,
            args.file_type.name(),
            args.output.display()
        );
    }

    Ok(())
}

/// True when `object_id` belongs in the requested bundle.
fn matches_file_type(file_type: FileType, object_id: u32) -> bool {
    let suffix = object_id & 0xffff;
    match file_type {
        FileType::Landblock => suffix == 0xFFFE || suffix == 0xFFFF,
        FileType::Envcell => (0x0100..0xFF00).contains(&suffix),
    }
}

/// Output file name for a bundle: full four-digit block hex.
fn output_file_name(block: u16) -> String {
    format!("0x{block:04X}.bin")
}

/// Resolve `client_cell_1.dat` from `--dat`, then `$DATS_DIR`, then `~/Dats`.
fn default_cell_dat_path() -> PathBuf {
    if let Some(dir) = std::env::var_os("DATS_DIR") {
        return PathBuf::from(dir).join("client_cell_1.dat");
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join("Dats").join("client_cell_1.dat");
    }
    PathBuf::from("client_cell_1.dat")
}

fn parse_landblock(text: &str) -> Result<u16, Box<dyn Error>> {
    let value = acdatservice::parse_file_id(text)
        .map_err(|error| format!("invalid --block {text:?}: {error}"))?;
    u16::try_from(value)
        .map_err(|_| format!("--block {text:?} is not a 16-bit landblock id").into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_landblock_metadata_suffixes() {
        assert!(matches_file_type(FileType::Landblock, 0xB34A_FFFF));
        assert!(matches_file_type(FileType::Landblock, 0xB34A_FFFE));
        assert!(!matches_file_type(FileType::Landblock, 0xB34A_0100));
    }

    #[test]
    fn classifies_envcell_suffix_range() {
        assert!(matches_file_type(FileType::Envcell, 0xB34A_0100));
        assert!(matches_file_type(FileType::Envcell, 0xB34A_FEFF));
        assert!(!matches_file_type(FileType::Envcell, 0xB34A_00FF));
        assert!(!matches_file_type(FileType::Envcell, 0xB34A_FF00));
        assert!(!matches_file_type(FileType::Envcell, 0xB34A_FFFF));
    }

    #[test]
    fn parses_hex_and_decimal_landblocks() {
        assert_eq!(parse_landblock("0xB34A").unwrap(), 0xB34A);
        assert_eq!(parse_landblock("45898").unwrap(), 0xB34A);
    }

    #[test]
    fn rejects_out_of_range_landblock() {
        assert!(parse_landblock("0x10000").is_err());
    }

    #[test]
    fn output_names_use_full_block_hex() {
        assert_eq!(output_file_name(0xB34A), "0xB34A.bin");
        assert_eq!(output_file_name(0x00AE), "0x00AE.bin");
    }

    #[test]
    fn collections_match_route_prefixes() {
        assert_eq!(FileType::Landblock.collection(), "landblocks");
        assert_eq!(FileType::Envcell.collection(), "envcells");
    }
}
