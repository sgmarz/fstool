//! File System Tool (fstool) Entrance Point
//!
//! © Stephen Marz
//! 8 June 2026
use crate::{
    args::parse_size_string,
    fs::MakeFileSystem,
    repl::{prompt_overwrite, run, state::State},
    source::{SOURCE_SEARCH_PATHS, run_source},
};
use args::{Args, FsType};
use clap::Parser;
use std::{fs::File, io, path::Path, process::ExitCode};

pub type MkfsFunction = fn(&mut File, u64) -> io::Result<()>;

/// # Main function for fstool.
fn main() -> ExitCode {
    // Use clap to parse command line arguments.
    let args = Args::parse();

    // Create is a one and go option. So, we return out of create_fs, which is a helper.
    if args.create {
        return create_fs(&args);
    }
    if args.umask > 0o777 {
        eprintln!("Umask must be a valid octal number between 000 and 777.");
        return ExitCode::FAILURE;
    }
    // For the interactive shell, we need to read from stdin, so we lock it and create a buffer for reading lines.
    let fl = match File::options()
        .read(true)
        .write(!args.no_write)
        .truncate(false)
        .open(&args.filename)
    {
        Ok(file) => file,
        Err(e) => {
            eprintln!("{}: {}", args.filename.display(), e);
            return ExitCode::FAILURE;
        }
    };
    let fs = match get_fs(fl) {
        Ok(fs) => fs,
        Err(e) => {
            eprintln!("{}: {}", args.filename.display(), e);
            return ExitCode::FAILURE;
        }
    };
    print!("{}: opened {}, ", args.filename.display(), fs.name());
    if !args.no_write {
        println!("read/write.");
    }
    else {
        println!("read-only.");
    }

    // If we get here, let's get the REPL started.
    let mut rl_editor = State::new(
        args.filename,
        !args.no_write,
        args.no_color,
        args.uid,
        args.gid,
        args.umask,
        fs,
    )
    .expect("unable to read file");

    // If the user specifies the source command, run them through the run command.
    // CLAP will ensure the no-source switch conflicts with the source switch.
    if let Some(source) = &args.source {
        if !source.exists() || !source.is_file() {
            eprintln!("{}: source file does not exist.", source.display());
            return ExitCode::FAILURE;
        }
        match run_source(source, rl_editor.helper_mut().unwrap()) {
            Ok(_) => {}
            Err(e) => {
                eprintln!("{}: {}", source.display(), e);
            }
        }
    }
    // Only try to find the source if the no-source switch was NOT specified.
    else if !args.no_source {
        let editor = rl_editor.helper_mut().unwrap();
        for source in SOURCE_SEARCH_PATHS.iter() {
            let source_path = Path::new(source);
            match run_source(source_path, editor) {
                Ok(_) => {
                    println!("Sourced '{}'.", source_path.display());
                    break;
                }
                _ => {}
            }
        }
    }

    repl::repl(&mut rl_editor);
    ExitCode::SUCCESS
}
/// ### Get the file system that is on the image.
///
/// #### Arguments
///
/// * `stream` - the File stream of the image.
///
/// #### Returns
///
/// * Ok(Box<dyn FileSystem>) on success, which is the FileSystem trait.
/// * Err(kind) on failure.
fn get_fs(mut stream: File) -> io::Result<Box<dyn fs::FileSystem>> {
    // Basically, look at each file system type until one checks valid.
    if let Ok(true) = crate::minix::check_valid(&mut stream) {
        return Ok(Box::new(crate::minix::MinixFileSystem::new(stream)?));
    }

    if let Ok(true) = crate::exfat::check_valid(&mut stream) {
        return Ok(Box::new(crate::exfat::ExfatFileSystem::new(stream)?));
    }

    // If none of the supported file systems checks valid, then we don't
    // have a suitable way to drive the image, or the image doesn't have
    // a file system on it.
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "no suitable filesystem found",
    ))
}

/// ### Find a way to format a new file system.
///
/// This function takes the file system type and returns the MkfsFunction pointer.
fn get_mkfs(fs_type: &FsType) -> MkfsFunction {
    match fs_type {
        &FsType::Minix => crate::minix::MinixFileSystem::mkfs,
        &FsType::Ext2 => crate::ext2::Ext2FileSystem::mkfs,
        &FsType::Exfat => crate::exfat::ExfatFileSystem::mkfs,
        &FsType::Mzfat => crate::mzfat::MzfatFileSystem::mkfs,
    }
}

fn create_fs(args: &Args) -> ExitCode {
    let file_size = match parse_size_string(&args.size) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("{}", e);
            return ExitCode::FAILURE;
        }
    };
    // Try to open the file read-only. This will see if the file already exists.
    // If it does, prompt the user and ask them if they want to overwrite it. If they do
    // then continue on, which will overwrite and truncate the old file.
    if File::options()
        .read(true)
        .create(false)
        .truncate(false)
        .open(&args.filename)
        .is_ok()
        && !prompt_overwrite(args.filename.to_str().unwrap())
    {
        return ExitCode::SUCCESS;
    }
    let mut fl = File::options()
        .read(false)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&args.filename)
        .expect("Failed to create file");
    fl.set_len(file_size).expect("Failed to set file size");
    let mkfs_fn = get_mkfs(&args.fs_type);
    if let Err(r) = mkfs_fn(&mut fl, file_size) {
        eprintln!("Failed to create filesystem: {}", r);
        // If we get here, we are the ones that created the file, so it's safe
        // to remove it if there is an error.
        let _ = std::fs::remove_file(&args.filename);
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

/// Round `value` up to the next multiple of `align` (must be a power of two).
#[inline]
pub fn align_up(value: u64, align: u64) -> u64 {
    debug_assert!(align > 0 && align.is_power_of_two());
    (value + align - 1) & !(align - 1)
}

mod args;
mod bitmap;
mod cache;
mod exfat;
mod ext2;
mod filetype;
mod fs;
mod minix;
mod mzfat;
mod path;
mod repl;
mod source;
mod stat;
mod terminal;
