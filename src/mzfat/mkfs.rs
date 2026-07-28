//! mzFAT Make File System
//!
//! © Stephen Marz
//! 8 June 2026
use super::{MzfatFileSystem, consts, entry::Entry, superblock::Superblock};
use crate::fs::MakeFileSystem;
use std::{
    fs::File,
    io::{self, Seek, SeekFrom, Write},
};

impl MakeFileSystem for MzfatFileSystem {
    fn mkfs(stream: &mut File, size: u64) -> io::Result<()> {
        let sb = {
            let mut s = Superblock::default();
            let blocks: u64 = size / consts::DEFAULT_BLOCK_SIZE;
            let block_shift = 10_u8;
            let fat_blocks =
                4 * (blocks + consts::DEFAULT_BLOCK_SIZE - 1) / consts::DEFAULT_BLOCK_SIZE;
            let bitmap_blocks = 1_u64
                .max((blocks + consts::DEFAULT_BLOCK_SIZE - 1) / consts::DEFAULT_BLOCK_SIZE / 8);

            s.magic = consts::MZFAT_MAGIC;
            s.block_shift = block_shift;
            s.fat_block_offset = 1;
            s.fat_num_blocks = fat_blocks as u16;
            s.bitmap_block_offset = s.fat_block_offset as u16 + s.fat_num_blocks;
            s.bitmap_num_blocks = bitmap_blocks as u16;
            s.root_block_offset = s.bitmap_block_offset + s.bitmap_num_blocks;
            s.num_data_blocks = blocks as u32 - s.root_block_offset as u32;
            s
        };
        let fat: Vec<u8> = {
            let size = sb.fat_num_blocks as u64 * consts::DEFAULT_BLOCK_SIZE;
            let mut f = vec![0_u32; (size as usize) >> 2];
            f[0] = consts::FAT_EOC;
            f.iter().flat_map(|x| x.to_le_bytes()).collect()
        };

        eprintln!("DEBUG: {:?}", sb);
        stream.seek(SeekFrom::Start(0))?;
        stream.write(sb.to_bytes())?;
        let root_inode = {
            let mut ri = Entry::default();
            ri
        };
        Ok(())
    }
}
