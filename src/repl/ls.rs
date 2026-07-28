//! List Command (ls) and Utilities
//!
//! © Stephen Marz
//! 8 June 2026
use super::{Args, State};
use crate::{
    fs::{FileSystem, FileType},
    path::build_path_deref,
    stat::make_combined_list,
    terminal,
};

const MAX_LINE_SIZE: usize = 25;
const COLUMN_SPACING: usize = 3;

fn run_ls(fs: &mut dyn FileSystem, parsed_path: &String, do_long: bool, all: bool, color: bool) {
    let mut tree = &{
        if let Some(old) = fs.get_tree() {
            old.clone()
        }
        else {
            println!("{}: I/O Error.", parsed_path);
            return;
        }
    };
    let path = parsed_path
        .split('/')
        .filter(|&x| !x.is_empty())
        .collect::<Vec<&str>>();
    // Find the parent
    if path.len() > 0 {
        for dir in path {
            if let Some(p) = tree.iter().find(|it| it.name().eq(dir)) {
                if !p.is_dir() {
                    println!("{}: Not a directory.", p.name());
                    return;
                }
                tree = p.next().unwrap();
            }
        }
    }
    if do_long {
        let mut list: Vec<(String, String, FileType)> = vec![];
        println!("Access      Links  UID   GID   Bytes      Inode  Name");
        println!("~~~~~~~~~~  ~~~~~  ~~~   ~~~   ~~~~~      ~~~~~  ~~~~");
        for item in tree {
            if !all && item.name().starts_with(".") {
                continue;
            }
            let inode_num = item.inode();
            let inode = fs.get_inode(inode_num).unwrap();
            let combined = make_combined_list(inode);
            let colored_name = mk_string(item.name(), inode.get_file_type(), color);
            let size_field = match inode.get_file_type() {
                FileType::CharacterDevice | FileType::BlockDevice => {
                    let (major, minor) = inode.get_node();
                    format!("{:<2},{:>2}", major, minor)
                }
                _ => {
                    format!("{}", inode.get_size())
                }
            };
            let s = if item.is_symlink() {
                format!(
                    "{}  {:<5}  {:<4}  {:<4}  {:<9}  {:>5}  {} -> {}",
                    combined,
                    inode.get_nlinks(),
                    inode.get_uid(),
                    inode.get_gid(),
                    size_field,
                    inode_num,
                    colored_name,
                    item.symlink_target().unwrap()
                )
            }
            else {
                format!(
                    "{}  {:<5}  {:<4}  {:<4}  {:<9}  {:>5}  {}",
                    combined,
                    inode.get_nlinks(),
                    inode.get_uid(),
                    inode.get_gid(),
                    size_field,
                    inode_num,
                    colored_name
                )
            };
            list.push((item.name().clone(), s, inode.get_file_type()));
        }
        list.sort_by(|(nx, _, lx), (ny, _, ly)| {
            let lxu = lx.sort_order();
            let lyu = ly.sort_order();
            if lxu == lyu {
                nx.cmp(ny)
            }
            else {
                lxu.cmp(&lyu)
            }
        });
        list.iter().for_each(|(_, out, _)| println!("{}", out));
    }
    else {
        // Not long
        let mut list: Vec<(String, FileType)> = vec![];
        tree.iter().for_each(|item| {
            if !all && item.name().starts_with(".") {
                return;
            }
            let inode = fs.get_inode(item.inode()).unwrap();
            list.push((item.name().clone(), inode.get_file_type()));
        });
        list.sort_by(|(nx, lx), (ny, ly)| {
            let lxu = lx.sort_order();
            let lyu = ly.sort_order();
            match lxu.cmp(&lyu) {
                std::cmp::Ordering::Equal => nx.cmp(ny),
                x => x,
            }
        });
        let mut cols: Vec<usize> = vec![];
        let mut line_size = 0;
        list.iter().enumerate().for_each(|(i, (name, _item_type))| {
            if line_size < MAX_LINE_SIZE {
                cols.push(0);
            }
            let x = i % cols.len();
            if name.len() > cols[x] {
                cols[x] = name.len();
                line_size += cols[x];
            }
        });
        list.iter().enumerate().for_each(|(i, (name, item_type))| {
            let x = i % cols.len();
            let padding = usize::max(cols[x] - name.len(), 0) + COLUMN_SPACING;
            let colored_name = mk_string(name, *item_type, color);
            print!("{}", colored_name);
            if x == cols.len() - 1 {
                println!();
            }
            else {
                (0..padding).for_each(|_| print!(" "));
            }
        });
        if cols.len() > 0 && list.len() % cols.len() != 0 {
            println!();
        }
    }
}

pub(super) fn do_ls(state: &mut State, args: Args) {
    // Split options and the path
    let (flag_args, path_args): (Vec<_>, Vec<_>) =
        args.iter().partition(|a: &&&str| a.starts_with('-'));
    // Set the options
    let long = flag_args.iter().any(|f: &&str| f.contains('l'));
    let all = flag_args.iter().any(|f: &&str| f.contains('a'));
    let color = !state.no_color && !flag_args.iter().any(|f: &&str| f.contains('c'));

    // If the path is empty, use the current directory. Otherwise, build the path and run ls on it.
    if path_args.is_empty() {
        run_ls(state.fs.as_mut(), &state.cwd.join("/"), long, all, color);
    }
    else {
        let built_path = match build_path_deref(state.fs.as_mut(), &state.cwd, &path_args[0]) {
            Ok(x) => x,
            Err(e) => {
                println!("{}", e);
                return;
            }
        };
        run_ls(state.fs.as_mut(), &built_path, long, all, color);
    }
}

pub fn mk_string(s: &str, file_type: FileType, color: bool) -> String {
    if !color {
        return s.to_string();
    }
    let color_code = get_ansi_color(file_type);
    format!("{}{}{}", color_code, s, terminal::ANSI_NORMAL)
}

pub fn get_ansi_color(file_type: FileType) -> &'static str {
    match file_type {
        FileType::Directory => terminal::bright::BLUE,
        FileType::CharacterDevice | FileType::BlockDevice => terminal::bright::YELLOW,
        FileType::Fifo => terminal::normal::YELLOW,
        FileType::Socket => terminal::bright::MAGENTA,
        FileType::Symlink => terminal::bright::CYAN,
        FileType::Invalid => terminal::bright::WHITE,
        FileType::Regular => terminal::ANSI_NORMAL,
    }
}
