//! Shell and Echo commands.
//!
//! Echo probably shouldn't be in here, but I didn't want to create another
//! file just for it.
//!
//! © Stephen Marz
//! 8 June 2026
use crate::{
    filetype::FileType,
    path::{build_path_deref, get_item},
    repl::{Args, State},
    source::run_source,
};
use std::{
    io::{Write, pipe},
    path::Path,
    process::Command,
};

pub(super) fn do_shell(_state: &mut State, args: Args) {
    if args.is_empty() {
        println!("Usage: shell <command> [command arguments]");
        return;
    }
    let cmd = args[0];
    let mut cmd_args = vec![];
    let mut i = 1;

    // Handle quoted arguments. If an argument starts with a quote, combine it with
    // subsequent arguments until we find one that ends with a quote. The quotes
    // themselves are not included in the final argument.
    'outer: while i < args.len() {
        let arg = &args[i];
        if arg.starts_with("\"") {
            let mut combined = arg[1..].to_string();
            if arg.ends_with("\"") && arg.len() > 1 {
                combined.pop();
                cmd_args.push(combined);
                i += 1;
                continue 'outer;
            }
            let mut j = i + 1;
            while j < args.len() {
                let next_arg = &args[j];
                combined.push(' ');
                combined.push_str(next_arg);
                if next_arg.ends_with("\"") {
                    combined.pop();
                    cmd_args.push(combined);
                    i = j + 1;
                    continue 'outer;
                }
                j += 1;
            }
        }
        cmd_args.push(arg.to_string());
        i += 1;
    }
    match Command::new(&cmd).args(cmd_args).spawn() {
        Ok(mut child) => {
            if let Err(x) = child.wait() {
                println!("{}: {}", cmd, x);
            }
        }
        Err(x) => {
            match x.kind() {
                std::io::ErrorKind::NotFound => println!("{}: command not found", cmd),
                _ => println!("{}: {}", cmd, x),
            }
        }
    }
}

pub(super) fn do_echo(_state: &mut State, args: Args) {
    // Don't use partition here because a dash can be echoed as long as it isn't the start.
    let mut flags = args.iter().take_while(|x| x.starts_with("-"));
    let args = args.iter().skip_while(|x| x.starts_with("-"));
    let output = args
        .map(|x| x.to_string())
        .collect::<Vec<String>>()
        .join(" ");
    let nl = if flags.any(|f| f.contains("n")) {
        ""
    }
    else {
        "\n"
    };
    print!("{output}{nl}");
}

pub(super) fn do_exec(state: &mut State, args: Args) {
    // TODO: Accept flags that allow the output of the command to be directed into a file on the image.
    const BUFFER_SIZE: u64 = 4096;
    let _flags = args
        .iter()
        .take_while(|x| x.starts_with("-"))
        .collect::<Vec<_>>();
    let args = args
        .iter()
        .skip_while(|x| x.starts_with("-"))
        .collect::<Vec<_>>();
    if args.len() < 2 {
        println!("Usage: exec <path> <command> [command arguments]");
        return;
    }
    let path = args[0];
    let command_str = args[1];
    let command_args = &args[2..];

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
    let size = inode.get_size();
    if size == 0 {
        println!("{}: File is empty.", fpath);
        return;
    }

    let (pipe_r, mut pipe_w) = match pipe() {
        Ok((r, w)) => (r, w),
        Err(e) => {
            println!("Unable to create pipe: {}", e);
            return;
        }
    };
    let buffer_size = u64::min(size, BUFFER_SIZE);
    let mut total_bytes_read = 0;
    let mut buffer = vec![0u8; buffer_size as usize];

    // Run the command, then send data to its stdin.
    let mut child = match Command::new(command_str)
        .args(command_args)
        .stdin(pipe_r)
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            println!("{e}");
            return;
        }
    };
    for _ in (0..size).step_by(buffer_size as usize) {
        if let Ok(bytes_read) = state.fs.read_file(p.inode(), total_bytes_read, &mut buffer) {
            match pipe_w.write_all(&buffer[..bytes_read as usize]) {
                Ok(_) => {}
                Err(e) => {
                    println!("{e}");
                    break;
                }
            }
            total_bytes_read += bytes_read;
        }
        else {
            break;
        }
    }
    // We need to drop the write end so the child stops waiting for input.
    drop(pipe_w);
    let _ = child.wait();
}

pub(super) fn do_source(state: &mut State, args: Args) {
    if args.is_empty() {
        println!("Usage: source <file>");
        return;
    }
    let path = Path::new(&args[0]);
    match run_source(&path, state) {
        Err(e) => {
            eprintln!("{}: {}", path.display(), e);
        }
        _ => {}
    }
}
