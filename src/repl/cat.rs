//! Printing Files Commands (e.g., cat, hexdump)
//!
//! © Stephen Marz
//! 8 June 2026
use super::{Args, State};
use crate::{
    fs::FileType,
    path::{build_path_deref, get_item},
};

const BUFFER_SIZE: u64 = 4096;
const DEFAULT_HEAD_BYTES: u64 = 512;
const NUM_BYTES_PER_LINE: usize = 16;

pub(super) fn do_cat(state: &mut State, args: Args) {
    if args.is_empty() {
        println!("Usage: cat <path>");
        return;
    }
    let fpath = match build_path_deref(state.fs.as_mut(), &state.cwd, args[0]) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("{}", e);
            return;
        }
    };
    let p = match get_item(state.fs.as_ref(), &fpath) {
        Ok(p) => p,
        Err(e) => {
            println!("{}", e);
            return;
        }
    };
    if p.is_dir() {
        println!("{}: Is a directory.", fpath);
        return;
    }
    let inode = match state.fs.get_inode(p.inode()) {
        Ok(i) => i,
        Err(e) => {
            println!("{}: I/O Error: {}", fpath, e);
            return;
        }
    };
    if inode.get_file_type() != FileType::Regular {
        println!("{}: Not a regular file.", fpath);
        return;
    }
    let size = inode.get_size();
    if size == 0 {
        // Zero size is not an error, just print nothing.
        return;
    }
    let buffer_size = u64::min(size, BUFFER_SIZE);
    let mut total_bytes_read = 0;
    let mut buffer = vec![0u8; buffer_size as usize];
    for _ in (0..size).step_by(buffer_size as usize) {
        if let Ok(bytes_read) = state.fs.read_file(p.inode(), total_bytes_read, &mut buffer) {
            String::from_utf8_lossy(&buffer[..bytes_read as usize])
                .chars()
                .for_each(|c| print!("{}", c));
            total_bytes_read += bytes_read;
        }
        else {
            if total_bytes_read > 0 {
                println!();
            }
            println!("{}: I/O Error.", fpath);
            return;
        }
    }
}

pub(super) fn do_catsymlink(state: &mut State, args: Args) {
    if args.is_empty() {
        println!("Usage: catsymlink <path>");
        return;
    }
    let split_path = args[0]
        .split('/')
        .filter(|&s| !s.is_empty())
        .collect::<Vec<&str>>();
    if split_path.is_empty() {
        println!("{}: Invalid path.", args[0]);
        return;
    }
    let path = if split_path.len() > 1 {
        split_path[..split_path.len() - 1].join("/")
    }
    else {
        ".".to_string()
    };

    let fpath = match build_path_deref(state.fs.as_mut(), &state.cwd, &path) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("{}", e);
            return;
        }
    };
    let symlink_path = fpath + split_path[split_path.len() - 1];
    let p = match get_item(state.fs.as_ref(), &symlink_path) {
        Ok(p) => p,
        Err(e) => {
            println!("{}", e);
            return;
        }
    };
    if p.is_dir() {
        println!("{}: Is a directory.", symlink_path);
        return;
    }
    let inode = match state.fs.get_inode(p.inode()) {
        Ok(i) => i,
        Err(e) => {
            println!("{}: I/O Error: {}", symlink_path, e);
            return;
        }
    };
    if inode.get_file_type() != FileType::Symlink {
        println!("{}: Not a symbolic link.", symlink_path);
        return;
    }
    if let Ok(link_value) = state.fs.read_symlink(p.inode()) {
        println!("{}", link_value);
    }
    else {
        println!("{}: I/O Error.", symlink_path);
        return;
    }
}

pub(super) fn do_catblock(state: &mut State, args: Args) {
    if args.is_empty() {
        println!("Usage: catblock <block number>");
        return;
    }
    let block_num = match args[0].parse::<u64>() {
        Ok(x) => x,
        Err(e) => {
            eprintln!("{}: {}", args[0], e);
            return;
        }
    };
    let block_size = state.fs.get_superblock().get_block_size();
    let mut buffer = vec![0u8; block_size as usize];
    if let Err(e) = state.fs.read_block(block_num, &mut buffer) {
        println!("catblock error: {}.", e);
        return;
    }
    for i in (0..buffer.len()).step_by(NUM_BYTES_PER_LINE) {
        let line_bytes = &buffer[i..];
        hex_dump_line(i as u64, line_bytes);
    }
    println!("{:<08x}", buffer.len());
}

pub(super) fn do_head(state: &mut State, args: Args) {
    let (num_bytes, path) = match args.len() {
        0 => {
            println!("Usage: head [bytes ({})] <path>", DEFAULT_HEAD_BYTES);
            return;
        }
        1 => (DEFAULT_HEAD_BYTES, args[0]),
        _ => {
            let bytes = match args[0].parse::<u64>() {
                Ok(x) => x,
                Err(e) => {
                    eprintln!("{}: {}", args[0], e);
                    return;
                }
            };
            (bytes, args[1])
        }
    };
    let fpath = match build_path_deref(state.fs.as_mut(), &state.cwd, path) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("{}", e);
            return;
        }
    };
    let p = match get_item(state.fs.as_ref(), &fpath) {
        Ok(p) => p,
        Err(e) => {
            println!("{}", e);
            return;
        }
    };
    if p.is_dir() {
        println!("{}: Is a directory.", fpath);
        return;
    }
    let inode = match state.fs.get_inode(p.inode()) {
        Ok(i) => i,
        Err(e) => {
            println!("{}: I/O Error: {}", fpath, e);
            return;
        }
    };
    if inode.get_file_type() != FileType::Regular {
        println!("{}: Not a regular file.", fpath);
        return;
    }
    let size = match u64::min(inode.get_size(), num_bytes) {
        // If there is no size, just return. It's not an error.
        0 => return,
        x => x,
    };
    let buffer_size = u64::min(size, BUFFER_SIZE);
    let mut total_bytes_read = 0;
    let mut buffer = vec![0u8; buffer_size as usize];
    for _ in (0..size).step_by(buffer_size as usize) {
        if let Ok(bytes_read) = state.fs.read_file(p.inode(), total_bytes_read, &mut buffer) {
            String::from_utf8_lossy(&buffer[..bytes_read as usize])
                .chars()
                .for_each(|c| print!("{}", c));
            total_bytes_read += bytes_read;
        }
        else {
            if total_bytes_read > 0 {
                println!();
            }
            println!("{}: I/O Error.", fpath);
            return;
        }
    }
    println!();
}

pub(super) fn do_hexdump(state: &mut State, args: Args) {
    const BUFFER_SIZE: u64 = 4096;
    const MAX_SIZE: u64 = BUFFER_SIZE * 10;
    if args.is_empty() {
        println!("Usage: hexdump [start-finish] <remote path>");
        return;
    }
    let (start, finish, name) = if args.len() > 1 {
        let start_finish = args[0].split_once('-');
        if start_finish.is_none() {
            if let Ok(bytes) = args[0].parse::<u64>() {
                (0, bytes, args[1])
            }
            else {
                println!("{}: could not parse start or finish.", args[0]);
                return;
            }
        }
        else {
            let (start, finish) = start_finish.unwrap();
            match (start.parse::<u64>(), finish.parse::<u64>()) {
                (Ok(s), Ok(f)) => (s, f, args[1]),
                (Ok(s), Err(_)) => (s, MAX_SIZE, args[1]),
                (Err(_), Ok(f)) => (0, f, args[1]),
                _ => {
                    println!("{}: could not convert start or finish.", args[0]);
                    return;
                }
            }
        }
    }
    else {
        (0, MAX_SIZE, args[0])
    };
    let remote_path = match build_path_deref(state.fs.as_mut(), &state.cwd, name) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("{}", e);
            return;
        }
    };
    let p = match get_item(state.fs.as_ref(), &remote_path) {
        Ok(p) => p,
        Err(e) => {
            println!("{}", e);
            return;
        }
    };
    if p.is_dir() {
        println!("{}: Is a directory.", name);
        return;
    }
    let inode = match state.fs.get_inode(p.inode()) {
        Ok(inode) => inode,
        Err(_) => {
            println!("{}: I/O Error.", name);
            return;
        }
    };

    match inode.get_file_type() {
        FileType::Regular => {}
        _ => {
            eprintln!("{}: Not a regular file.", &remote_path);
            return;
        }
    }

    let size = match inode.get_size() {
        0 => return,
        x => u64::min(finish.saturating_sub(start), x),
    };
    let mut buffer = vec![0u8; NUM_BYTES_PER_LINE];
    for i in (0..size).step_by(NUM_BYTES_PER_LINE) {
        let offset = i + start;
        match state.fs.read_file(p.inode(), offset, &mut buffer) {
            Ok(num) => {
                let num = u64::min(num, size - (offset - start));
                hex_dump_line(offset, &buffer[..num as usize]);
            }
            // TODO: This seems like a hack, maybe calculate better above with the sizes and break the loop there instead of relying on this error to break the loop.
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                break;
            }
            Err(e) => {
                println!("{}", e);
                return;
            }
        }
    }
    println!("{:<08x}", size);
}

fn hex_dump_line(line_offset: u64, buffer: &[u8]) {
    let num_bytes = usize::min(NUM_BYTES_PER_LINE, buffer.len());
    let rem = NUM_BYTES_PER_LINE - num_bytes;
    print!("{:>08x}  ", line_offset);
    (0..usize::min(8, num_bytes)).for_each(|i| {
        print!("{:02x} ", buffer[i]);
    });
    print!("   ");
    (usize::min(8, num_bytes)..num_bytes).for_each(|i| {
        print!("{:02x} ", buffer[i]);
    });
    (0..rem).for_each(|_| print!("   "));
    print!("  |");
    let to_printable = |baddr: &u8| {
        match *baddr {
            b if b.is_ascii_graphic() => b as char,
            b if b == b' ' => ' ',
            _ => '.',
        }
    };
    buffer[..num_bytes]
        .iter()
        .map(to_printable)
        .for_each(|b| print!("{}", b));
    println!("|");
}
