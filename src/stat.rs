//! In-memory file metadata (stats)
//!
//! © Stephen Marz
//! 8 June 2026
#![allow(dead_code)]
use super::filetype::FileType;
use crate::fs::InodeOperations;
use chrono::{Local, TimeZone};

#[derive(Debug)]
pub struct Stat {
    pub dev: u64,
    pub ino: u64,
    pub mode: u32,
    pub nlink: u64,
    pub uid: u32,
    pub gid: u32,
    pub rdev: u64,
    pub size: u64,
    pub blksize: u64,
    pub blocks: u64,
    pub atime: u64,
    pub mtime: u64,
    pub ctime: u64,
}

impl Stat {
    pub fn permissions(&self) -> u32 {
        self.mode & 0o777
    }

    pub fn file_type(&self) -> FileType {
        (self.mode as u16).into()
    }

    pub fn major(&self) -> u32 {
        (self.rdev >> 8) as u32
    }

    pub fn minor(&self) -> u32 {
        (self.rdev & 0xFF) as u32
    }
}

/// POSIX permissions and masking constants.
pub const S_IFMT: u16 = 0o170000;
pub const S_IFSOCK: u16 = 0o140000;
pub const S_IFLNK: u16 = 0o120000;
pub const S_IFREG: u16 = 0o100000;
pub const S_IFBLK: u16 = 0o60000;
pub const S_IFDIR: u16 = 0o40000;
pub const S_IFCHR: u16 = 0o20000;
pub const S_IFIFO: u16 = 0o10000;
// pub const S_ISUID: u16 = 0o4000;
// pub const S_ISGID: u16 = 0o2000;
// pub const S_ISVTX: u16 = 0o1000;

pub const S_IRWXU: u16 = 0o700;
pub const S_IRUSR: u16 = 0o400;
pub const S_IWUSR: u16 = 0o200;
pub const S_IXUSR: u16 = 0o100;

pub const S_IRWXG: u16 = 0o70;
pub const S_IRGRP: u16 = 0o40;
pub const S_IWGRP: u16 = 0o20;
pub const S_IXGRP: u16 = 0o10;

pub const S_IRWXO: u16 = 0o7;
pub const S_IROTH: u16 = 0o4;
pub const S_IWOTH: u16 = 0o2;
pub const S_IXOTH: u16 = 0o1;

// Helper function to make the combined list of file type and permissions
pub fn make_combined_list(inode: &dyn InodeOperations) -> String {
    let m = inode.get_mode();
    let mut s = String::with_capacity(10);

    s.push(match inode.get_file_type() {
        FileType::Directory => 'd',
        FileType::Symlink => 'l',
        FileType::BlockDevice => 'b',
        FileType::CharacterDevice => 'c',
        FileType::Fifo => 'p',
        FileType::Socket => 's',
        _ => '-',
    });

    for (r, w, x) in [
        (S_IRUSR, S_IWUSR, S_IXUSR),
        (S_IRGRP, S_IWGRP, S_IXGRP),
        (S_IROTH, S_IWOTH, S_IXOTH),
    ] {
        s.push(if (m & r) != 0 { 'r' } else { '-' });
        s.push(if (m & w) != 0 { 'w' } else { '-' });
        s.push(if (m & x) != 0 { 'x' } else { '-' });
    }

    s
}

pub fn unix_to_human(timestamp: u64) -> String {
    match Local.timestamp_opt(timestamp as i64, 0).single() {
        Some(datetime) => datetime.format("%Y-%m-%d %H:%M:%S %z").to_string(),
        None => "invalid".to_string(),
    }
}

/// Parse a chmod mode string and return the new 12-bit permission value.
///
/// Accepts both octal literals (`"755"`, `"4755"`) and one or more
/// comma-separated symbolic clauses (`"u+x"`, `"a=rw"`, `"u+s,g-w,o="`).
///
/// # Symbolic grammar
/// ```text
/// mode   ::= clause (',' clause)*
/// clause ::= who* op perm*
/// who    ::= 'u' | 'g' | 'o' | 'a'          (default: 'a' when omitted)
/// op     ::= '+' | '-' | '='
/// perm   ::= 'r' | 'w' | 'x' | 'X' | 's' | 't'
/// ```
///
/// `X` sets the execute bits only when `is_dir` is true **or** at least one
/// execute bit is already set in `current_bits` (POSIX behaviour).
pub fn parse_symbolic_mode(mode_str: &str, current_bits: u16, is_dir: bool) -> Result<u16, String> {
    // Octal mode? Just parse it.
    if mode_str.chars().all(|c| matches!(c, '0'..='7')) {
        return u16::from_str_radix(mode_str, 8)
            .map(|m| m & 0o7777)
            .map_err(|_| format!("{}: invalid octal mode", mode_str));
    }

    // Symbolic parsing
    let mut result = current_bits & 0o7777;

    for clause in mode_str.split(',') {
        if clause.is_empty() {
            continue;
        }

        let bytes = clause.as_bytes();
        let mut i = 0;

        // 1. Parse the "who" prefix.
        let mut who = 0u16;
        while i < bytes.len() {
            let added = match bytes[i] {
                b'u' => 0o700u16,
                b'g' => 0o070u16,
                b'o' => 0o007u16,
                b'a' => 0o777u16,
                _ => break,
            };
            who |= added;
            i += 1;
        }
        // No explicit who. Treat as 'a' (all).
        if who == 0 {
            who = 0o777;
        }

        // 2. Parse the operator.
        let op = match bytes.get(i) {
            Some(&b'+') => {
                i += 1;
                '+'
            }
            Some(&b'-') => {
                i += 1;
                '-'
            }
            Some(&b'=') => {
                i += 1;
                '='
            }
            _ => return Err(format!("chmod: '{}': missing or invalid operator", clause)),
        };

        // Parse the permission characters.
        let mut perm_mask = 0u16;
        while i < bytes.len() {
            let bits: u16 = match bytes[i] {
                // rwx: mask down to only the bits covered by `who`
                b'r' => 0o444 & who,
                b'w' => 0o222 & who,
                b'x' => 0o111 & who,
                // X: conditional execute
                b'X' => {
                    if is_dir || (result & 0o111 != 0) {
                        0o111 & who
                    }
                    else {
                        0
                    }
                }
                // s: setuid when 'u' in who, setgid when 'g' in who
                b's' => {
                    let mut s = 0u16;
                    if who & 0o700 != 0 {
                        s |= 0o4000;
                    }
                    if who & 0o070 != 0 {
                        s |= 0o2000;
                    }
                    s
                }
                // t: sticky bit (who is irrelevant for sticky)
                b't' => 0o1000,
                c => {
                    return Err(format!(
                        "chmod: '{}': invalid permission character '{}'",
                        clause, c as char
                    ));
                }
            };
            perm_mask |= bits;
            i += 1;
        }

        // 4. Apply the operator.
        match op {
            '+' => {
                result |= perm_mask;
            }
            '-' => {
                result &= !perm_mask;
            }
            '=' => {
                // Build a mask of every bit that `who` can affect:
                //   • the rwx bits covered by who
                //   • setuid  if 'u' is in who
                //   • setgid  if 'g' is in who
                //   • sticky  if 'o' is in who (or 'a')
                let mut clear_mask = who; // rwx bits
                if who & 0o700 != 0 {
                    clear_mask |= 0o4000;
                }
                if who & 0o070 != 0 {
                    clear_mask |= 0o2000;
                }
                if who & 0o007 != 0 {
                    clear_mask |= 0o1000;
                }

                result = (result & !clear_mask) | perm_mask;
            }
            _ => unreachable!(),
        }
    }

    Ok(result & 0o7777)
}
