//! Ext2 File System
//!
//! © Stephen Marz
//! 8 June 2026
pub mod consts;
pub mod fs;
pub mod inode;
pub mod mkfs;
pub mod superblock;

// Exports
pub use fs::Ext2FileSystem;
pub use inode::Inode;
pub use superblock::Superblock;
