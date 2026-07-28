//! Minix 3 File System Support Routines
//!
//! © Stephen Marz
//! 8 June 2026
use super::build::buildfs;
use super::{Bitmap, MinixFileSystem, Superblock};
use crate::cache::BlockCache;
use std::{fs::File, io};

impl MinixFileSystem {
    pub fn new(mut stream: File) -> io::Result<Self> {
        let superblock = Superblock::from_stream(&mut stream)?;

        if !superblock.is_valid() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid superblock magic number",
            ));
        }

        buildfs(Self {
            superblock,
            imap: Bitmap::default(),
            zmap: Bitmap::default(),
            inodes: Vec::default(),
            tree: Vec::default(),
            stream,
            bcache: BlockCache::default(),
        })
    }
}

pub fn check_valid<'a, U: io::Read + io::Seek>(stream: &'a mut U) -> io::Result<bool> {
    let sb = Superblock::from_stream(stream)?;
    Ok(sb.is_valid())
}
