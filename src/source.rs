//! File System Tool (fstool) Entrance Point
//!
//! © Stephen Marz
//! 8 June 2026
use crate::{State, run};
use std::{
    env::home_dir,
    fs::File,
    io::{self, BufRead, BufReader},
    path::Path,
};
// Search paths to find the RC file to source at startup, in order of precedence.
// The first one found is used. If the user specifies in the command line arguments,
// this is never consulted.
pub(super) const SOURCE_SEARCH_PATHS: [&str; 15] = [
    "fstoolrc",
    ".fstoolrc",
    "fstool.rc",
    ".fstool.rc",
    "~/.config/fstoolrc",
    "~/.config/.fstoolrc",
    "~/.config/fstool.rc",
    "~/.config/.fstool.rc",
    "~/.config/fstoolrc",
    "/etc/fstoolrc",
    "/etc/fstool.rc",
    "/usr/etc/fstoolrc",
    "/usr/etc/fstool.rc",
    "/usr/local/etc/fstoolrc",
    "/usr/local/etc/fstool.rc",
];
/// ### Run commands from a file rather than stdin.
///
/// This is typically used in conjunction with the `-s/--source`
/// command line argument.
///
/// #### Arguments
///
/// * `source` the Path of the file to source.
/// * `state`  the mutable internal state.
///
/// #### Returns
///
/// * `true` if the file was opened and read successfully.
/// * `false` otherwise
///
/// #### Issues
///
/// * Since `source` is a command, infinitely recursive source command are possible.
pub fn run_source(source: &Path, state: &mut State) -> io::Result<()> {
    // First, see if the file exists. If it does, open it and attach it
    // to a buffered file reader so we can go line-by-line.
    // Otherwise, if the file cannot be opened, give an error message and return.
    // We do this so that the program doesn't exist on this failure, but rather
    // tells the user and continues.
    let source = if source.is_absolute() {
        source.to_path_buf()
    }
    else {
        // If the path is not absolute, we assume it's relative to the user's home directory.
        if source.starts_with("~") {
            let hd = match home_dir() {
                Some(home) => home,
                None => {
                    // eprintln!("Failed to get home directory, cannot resolve relative path.");
                    return Err(io::Error::new(
                        io::ErrorKind::NotConnected,
                        "unable to resolve home directory",
                    ));
                }
            };
            source
                .to_string_lossy()
                .replacen("~", hd.to_str().unwrap_or(""), 1)
                .into()
        }
        else {
            source.to_path_buf()
        }
    };
    let mut br = match File::open(&source) {
        Ok(br) => BufReader::new(br),
        Err(x) => return Err(x),
    };
    let mut buf = String::new();
    let mut line_no = 0_usize;
    // println!("{}: reading as source.", source.display());
    while let Ok(bytes) = br.read_line(&mut buf) {
        line_no += 1;
        // Bytes being 0 here indicates EOF.
        if bytes == 0 {
            break;
        }
        let line = buf.trim();
        // Skip empty lines
        if line.is_empty() {
            buf.clear();
            continue;
        }
        // Allow comments
        if line.starts_with('#') {
            buf.clear();
            continue;
        }
        // Source-only command "break" to stop sourcing the file.
        if line == "break" {
            break;
        }
        // After all that, run the line.
        if !run(state, line) {
            println!(
                "   {}: line {} failed ({}).",
                source.display(),
                line_no,
                line
            );
        }
        buf.clear();
    }
    Ok(())
}
