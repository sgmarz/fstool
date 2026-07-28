//! Ext2 File System Creation
//!
//! © Stephen Marz
//! 8 June 2026
use super::Ext2FileSystem;
use crate::fs::MakeFileSystem;
use std::fs::File;
use std::io::{self, Write};
use std::time::{SystemTime, UNIX_EPOCH};

impl MakeFileSystem for Ext2FileSystem {
    fn mkfs(stream: &mut File, size: u64) -> io::Result<()> {
        // mkfs_ext2(stream, size)
        unimplemented!()
    }
}

#[allow(dead_code)]
fn mkfs_ext2(stream: &mut File, size: u64) -> io::Result<()> {
    stream.write_all(b"EXT2")?;
    Ok(())
}
