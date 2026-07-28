//! ExFAT File System.
//!
//!
//! © Stephen Marz
//! 8 June 2026
mod consts;
mod entry;
mod fs;
mod inode;
pub mod mbs;
mod mkfs;
mod timestamp;

use crate::{
    bitmap::Bitmap,
    cache::{BlockCache, Tree},
    fs::SuperblockOperations,
};
use mbs::MainBootSector;
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom},
};

pub struct ExfatFileSystem {
    pub mbs: MainBootSector, // Cached Main Boot Sector (MBS)
    pub fat: Vec<u32>,       // Cached exFAT file allocation table
    pub bitmap: Bitmap,
    pub tree: Tree,
    pub stream: File,
    pub bcache: BlockCache,
}

impl ExfatFileSystem {
    pub fn new(mut stream: File) -> io::Result<Self> {
        let bcache = BlockCache::default();
        let mbs = {
            let mut buffer = [0_u8; consts::MAIN_BOOT_SECTOR_SIZE];
            stream.seek(SeekFrom::Start(0))?;
            stream.read_exact(&mut buffer)?;
            MainBootSector::from_bytes(&buffer)
        };
        let fat = {
            let offset = mbs.sectors_to_bytes(mbs.fat_offset as u64);
            let len = mbs.sectors_to_bytes(mbs.fat_length as u64);
            let mut buffer = vec![0_u8; len as usize];
            stream.seek(SeekFrom::Start(offset))?;
            stream.read_exact(&mut buffer)?;
            buffer
                .chunks_exact(4)
                .map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()))
                .collect::<Vec<u32>>()
        };
        let bitmap = {
            let mut cluster = vec![0_u8; mbs.get_block_size() as usize];
            let mut current_cluster = mbs.root_directory_cluster;
            let bitmap_entry = 'bitmapouter: loop {
                if current_cluster >= consts::EXFAT_EOC_MIN {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "no bitmap in ExFAT file system",
                    ));
                }
                let start = mbs.cluster_byte_offset(current_cluster);
                stream.seek(SeekFrom::Start(start))?;
                stream.read_exact(&mut cluster)?;
                for entry in cluster.chunks_exact(32) {
                    if entry[0] == consts::TYPE_ALLOC_BITMAP {
                        // Found it! Read the bitmap into map and map_size
                        break 'bitmapouter entry::AllocBitmapEntry::from_bytes(entry);
                    }
                }
                current_cluster = fat[current_cluster as usize];
            };
            let cluster_count = mbs.cluster_count as usize;
            let bitmap_calc_size = (cluster_count + 7) / 8;
            let map_size = bitmap_entry.get_data_length() as usize;
            debug_assert!(bitmap_calc_size == map_size);
            // Read the bitmap cluster(s)
            let map = {
                let mut ret: Vec<u8> = Vec::new();
                let mut current_cluster = bitmap_entry.get_first_cluster();
                while current_cluster < consts::EXFAT_EOC_MIN {
                    let mut cluster = vec![0_u8; mbs.get_block_size() as usize];
                    let start = mbs.cluster_byte_offset(current_cluster);
                    stream.seek(SeekFrom::Start(start))?;
                    stream.read_exact(&mut cluster)?;
                    ret.extend(cluster.iter());
                    current_cluster = fat[current_cluster as usize];
                }
                ret
            };
            Bitmap::take(map, map_size)
        };
        Ok(Self {
            mbs,
            fat,
            bitmap,
            tree: vec![],
            stream,
            bcache,
        })
    }
}

pub fn check_valid<'a, U: io::Read + io::Seek>(stream: &'a mut U) -> io::Result<bool> {
    let mbs = MainBootSector::from_bytes(&{
        let mut buf = [0u8; consts::MAIN_BOOT_SECTOR_SIZE];
        stream.seek(io::SeekFrom::Start(0))?;
        stream.read_exact(&mut buf)?;
        buf
    });
    Ok(mbs.is_valid())
}
