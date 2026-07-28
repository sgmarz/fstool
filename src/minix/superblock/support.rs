//! Superblock Support Implementation
//!
//! © Stephen Marz
//! 8 June 2026
use super::super::consts::*;
use super::Superblock;
use std::io;
use std::slice::from_raw_parts_mut;

impl<'a> Superblock {
    pub const fn new() -> Self {
        Self {
            num_inodes: 0,
            pad0: 0,
            imap_blocks: 0,
            zmap_blocks: 0,
            first_data_zone: 0,
            log_zone_size: 0,
            pad1: 0,
            max_size: MAX_FILE_SIZE,
            num_zones: 0,
            magic: MINIX_MAGIC,
            pad2: 0,
            block_size: DEFAULT_BLOCK_SIZE,
            disk_version: DEFAULT_DISK_VERSION,
        }
    }

    pub const fn new_with(
        num_inodes: u32,
        imap_blocks: u16,
        zmap_blocks: u16,
        first_data_zone: u16,
        num_zones: u32,
        block_size: u16,
    ) -> Self {
        Self {
            num_inodes,
            pad0: 0,
            imap_blocks,
            zmap_blocks,
            first_data_zone,
            log_zone_size: 0,
            pad1: 0,
            max_size: MAX_FILE_SIZE,
            num_zones,
            magic: MINIX_MAGIC,
            pad2: 0,
            block_size,
            disk_version: DEFAULT_DISK_VERSION,
        }
    }

    pub fn from_stream<T>(stream: &'a mut T) -> io::Result<Self>
    where
        T: io::Read + io::Seek,
    {
        stream.seek(io::SeekFrom::Start(BOOT_BLOCK_BYTES))?;
        let mut superblock = Superblock::new();
        let mut superblock_buffer = unsafe {
            from_raw_parts_mut(
                (&mut superblock as *mut Superblock) as *mut u8,
                SUPERBLOCK_BYTES as usize,
            )
        };

        stream.read_exact(&mut superblock_buffer)?;

        Ok(superblock)
    }

    pub fn is_valid(&self) -> bool {
        self.magic == MINIX_MAGIC
    }
}
