//! Minix 3 File System Building Routines
//!
//! Used to build the on-disk file system to the in-memory file system.
//!
//! © Stephen Marz
//! 8 June 2026
use super::super::consts;
use super::MinixFileSystem;
use super::{DirEntry, Inode};
use crate::cache::{Item, Tree};
use crate::minix::Bitmap;
use std::io::{self, Read, Seek, SeekFrom};
use std::slice::from_raw_parts_mut;

pub(super) fn buildfs(mut fs: MinixFileSystem) -> io::Result<MinixFileSystem> {
    let num_inodes = (fs.superblock.num_inodes as usize + 7) / 8;
    let mut inode_bitmap = vec![0; num_inodes];
    fs.stream
        .seek(SeekFrom::Start(2 * fs.superblock.block_size as u64))?;
    fs.stream.read_exact(&mut inode_bitmap)?;
    let num_zones = (fs.superblock.num_zones as usize + 7) / 8;
    let mut zone_bitmap = vec![0; num_zones];
    fs.stream.seek(SeekFrom::Start(
        (2 + fs.superblock.imap_blocks as u64) * fs.superblock.block_size as u64,
    ))?;
    fs.stream.read_exact(&mut zone_bitmap)?;
    let inode_bytes = fs.superblock.num_inodes as usize * consts::INODE_BYTES as usize;
    let mut inodes = vec![0; inode_bytes];
    fs.stream.read_exact(&mut inodes)?;
    let inodes = unsafe {
        from_raw_parts_mut(
            inodes.as_mut_ptr() as *mut Inode,
            fs.superblock.num_inodes as usize,
        )
        .to_vec()
    };
    fs.tree = build_tree(
        &mut fs.stream,
        &inodes,
        consts::ROOT_INODE as usize,
        fs.superblock.block_size as usize,
        consts::ROOT_INODE as usize,
    )?;
    fs.imap = Bitmap::take(inode_bitmap, fs.superblock.num_inodes as usize);
    fs.zmap = Bitmap::take(zone_bitmap, fs.superblock.num_zones as usize);
    fs.inodes = inodes;
    Ok(fs)
}

fn build_tree<'a, T: io::Read + io::Seek>(
    stream: &mut T,
    inodes: &Vec<Inode>,
    inode_num: usize,
    block_size: usize,
    parent_inode: usize,
) -> io::Result<Tree> {
    assert!(inode_num >= 1);
    let inode_index = inode_num - 1;
    let inode = &inodes[inode_index];
    assert!(inode.is_dir());

    let mut tree = Tree::new();
    // Direct pointers, first
    for i in 0..consts::INDIRECT_ZONE {
        let b = inode.zones[i] as usize;
        if b == 0 {
            continue;
        }
        let entries = read_dir_entries(stream, b, block_size)?;
        for e in &entries {
            let next_inode_num = e.inode();
            if next_inode_num == 0 {
                continue;
            }
            if e.name() == "." {
                tree.push(Item::new_dir(e.clone_name(), e.inode(), vec![]));
                continue;
            }
            if e.name() == ".." {
                tree.push(Item::new_dir(e.clone_name(), parent_inode as u64, vec![]));
                continue;
            }
            let next_inode = &inodes[next_inode_num as usize - 1];

            if next_inode.is_dir() {
                let subtree = build_tree(
                    stream,
                    inodes,
                    next_inode_num as usize,
                    block_size,
                    inode_num,
                )?;
                tree.push(Item::new_dir(e.clone_name(), e.inode(), subtree));
            }
            else if next_inode.is_symlink() {
                if next_inode_num == 0 {
                    eprintln!("WARNING: {}: symlink with no target", e.name());
                    continue;
                }
                let block_num = next_inode.zones[0] as u64;
                stream.seek(SeekFrom::Start(block_num * block_size as u64))?;
                let sym_size = usize::min(next_inode.size as usize, consts::MAX_SYMLINK_SIZE);
                let mut link_target = vec![0u8; sym_size];
                stream.read_exact(&mut link_target)?;
                link_target.retain(|&c| c != 0);
                let link_string = String::from_utf8_lossy(&link_target).to_string();
                tree.push(Item::new_symlink(
                    e.clone_name(),
                    next_inode_num,
                    link_string,
                ));
            }
            else {
                tree.push(Item::new_file(e.clone_name(), e.inode()));
            }
        }
    }
    Ok(tree)
}

fn read_dir_entries<'a, T>(
    stream: &'a mut T,
    block: usize,
    block_size: usize,
) -> io::Result<Vec<Item>>
where
    T: io::Read + io::Seek,
{
    let mut block_data = vec![0u8; block_size];
    stream.seek(SeekFrom::Start(block as u64 * block_size as u64))?;
    stream.read_exact(&mut block_data)?;
    let entries = unsafe {
        from_raw_parts_mut(
            &mut block_data[0] as *mut u8 as *mut DirEntry,
            block_data.len() / consts::DIR_ENTRY_BYTES as usize,
        )
    };
    let ret = entries
        .iter()
        .map(|&direntry| direntry_to_entry(direntry))
        .collect::<Vec<Item>>();
    Ok(ret)
}

fn direntry_to_entry(direntry: DirEntry) -> Item {
    let name = direntry
        .name
        .iter()
        .filter_map(|&c| if c == 0 { None } else { Some(c as char) })
        .collect::<String>();
    Item::new_file(name, direntry.inode as u64)
}
