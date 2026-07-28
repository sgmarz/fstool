//! Argument Parsing
//!
//! © Stephen Marz
//! 8 June 2026
use std::path::PathBuf;

use clap::{Parser, ValueEnum};

#[derive(Parser)]
#[command(version, about, long_about = None)]
/// File system tool for creating and interacting with file systems.
pub struct Args {
    /// The filename of a filesystem image.
    pub filename: PathBuf,

    /// Create a new filesystem on the specified file.
    #[arg(short, long, default_value_t = false)]
    pub create: bool,

    /// Override source search, and source the given file at start.
    #[arg(short, long, value_name = "FILE")]
    pub source: Option<PathBuf>,

    /// Don't source any files at startup, even if they exist.
    #[arg(long, default_value_t = false, conflicts_with = "source")]
    pub no_source: bool,

    /// Filesystem to create.
    #[arg(short, long, value_enum, default_value_t = FsType::Minix)]
    pub fs_type: FsType,

    /// Open the file in read-only mode.
    #[arg(short = 'r', long = "read-only", default_value_t = false)]
    pub no_write: bool,

    /// Size of the filesystem to create, in bytes.
    #[arg(short = 'z', long, requires = "create", default_value = "32M")]
    pub size: String,

    /// Disable colored output.
    #[arg(short, long = "no-color", default_value_t = false)]
    pub no_color: bool,

    /// The user ID to use when creating a new filesystem.
    #[arg(short, long, default_value_t = 0)]
    pub uid: u32,

    /// The group ID to use when creating a new filesystem.
    #[arg(short, long, default_value_t = 0)]
    pub gid: u32,

    /// The default umask to use, in octal.
    #[arg(short = 'k', long, value_parser = parse_octal, default_value = "022")]
    pub umask: u16,
}

fn parse_octal(s: &str) -> Result<u16, String> {
    u16::from_str_radix(s, 8).map_err(|e| format!("Invalid octal number: {}", e))
}

#[derive(ValueEnum, Clone, Debug)]
pub enum FsType {
    Minix,
    Ext2,
    Exfat,
    Mzfat,
}

/// ### Parse a size string to support suffixes.
///
/// The normal Rust parse only supports strings that contain digits,
/// but for device sizes, this can get difficult especially if we need to
/// keep things as a power of 2. So, instead, we can accept suffixes, such as
/// `K` (kibibyte), `M` (mibibyte), and `G` (gibibyte), for orders of
/// 2^10, 2^20, and 2^30, respectively.
///
/// Returns the same result as parse, which is the value wrapped in `Ok()` or
/// the `Err()`.
pub fn parse_size_string(string: &String) -> Result<u64, std::num::ParseIntError> {
    // See if we have a suffix first. This simplifies error checking since we can
    // safely slice the string to len() - 1 without worrying about overlapping
    // subtraction.
    if string.len() >= 2 {
        // kibibyte (2^10)
        if string.ends_with('K') || string.ends_with('k') {
            return match string[..string.len() - 1].parse::<u64>() {
                Ok(x) => Ok(x << 10),
                Err(e) => Err(e),
            };
        }
        // mebibibyte (2^20)
        else if string.ends_with('M') || string.ends_with('m') {
            return match string[..string.len() - 1].parse::<u64>() {
                Ok(x) => Ok(x << 20),
                Err(e) => Err(e),
            };
        }
        // gibibyte (2^30)
        else if string.ends_with('G') || string.ends_with('g') {
            return match string[..string.len() - 1].parse::<u64>() {
                Ok(x) => Ok(x << 30),
                Err(e) => Err(e),
            };
        }
        else if string.ends_with('T') || string.ends_with('t') {
            return match string[..string.len() - 1].parse::<u64>() {
                Ok(x) => Ok(x << 40),
                Err(e) => Err(e),
            };
        }
    }
    // Other sizes will fall through here too, but those will fail on parse().
    // Otherwise, if the string is just digits, this will succeed.
    string.parse::<u64>()
}
