//! Minix 3 File System Directory Entry
//!
//! © Stephen Marz
//! 8 June 2026
use super::consts::DIR_ENTRY_NAME_SIZE;

pub mod support;

#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct DirEntry {
    pub inode: u32,
    pub name: [u8; DIR_ENTRY_NAME_SIZE],
}
