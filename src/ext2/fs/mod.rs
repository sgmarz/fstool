//! Ext2 File System Implementation
//!
//! © Stephen Marz
//! 8 June 2026
use super::superblock::Superblock;
use crate::bitmap::Bitmap;
use crate::cache::Tree;
use std::io;

pub struct Ext2FileSystem {
    pub superblock: Superblock,
    pub imap: Bitmap,
    pub bmap: Bitmap,
    // pub inodes: Vec<Inode>,
    pub tree: Tree,
}
