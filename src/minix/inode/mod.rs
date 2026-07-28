//! Minix 3 Filesystem Inode
//!
//! © Stephen Marz
//! 8 June 2026
use crate::minix::consts::NUM_ZONE_POINTERS;

pub mod support;

#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct Inode {
    pub mode: u16,
    pub nlinks: u16,
    pub uid: u16,
    pub gid: u16,
    pub size: u32,
    pub atime: u32,
    pub mtime: u32,
    pub ctime: u32,
    pub zones: [u32; NUM_ZONE_POINTERS],
}
