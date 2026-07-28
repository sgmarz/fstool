//! ExFAT File System Inodes
//!
//! ExFAT has no formal inode structure, but this module defines a synthesized `ExfatInode`
//! struct that implements the `InodeOperations` trait.  This allows us to treat ExFAT files
//! uniformly with other file systems in the rest of the tool, even though the underlying
//! metadata is stored differently (in directory entries rather than an inode table).
//!
//! © Stephen Marz
//! 8 June 2026
use super::timestamp::ExfatTimestamp;
use crate::{filetype::FileType, fs::InodeOperations};

/// A synthesized "inode" for ExFAT.
///
/// ExFAT has no inode table. All metadata lives in the directory entry set.
/// This struct is populated when a file is opened and satisfies the same
/// `InodeOperations` interface used throughout the rest of the tool.
///
/// The "inode number" is the first cluster of the file.  This is the natural
/// unique identifier: two directory entries with the same first cluster refer
/// to the same data (hard links, which ExFAT doesn't formally support, but
/// the cluster number is still a valid key).
///
/// For empty files (size 0), the first cluster is 0 and a synthetic inode
/// number is derived from the directory entry's byte offset instead.
#[derive(Debug)]
pub struct ExfatInode {
    /// First cluster of the file's data (0 if empty).
    pub first_cluster: u32,
    /// Number of valid bytes in the file.
    pub valid_data_length: u64,
    /// Allocated size (always a multiple of cluster size).
    pub data_length: u64,
    /// File attribute flags.
    pub file_type: FileType,
    /// `NoFatChain` flag from the Stream Extension.
    pub no_fat_chain: bool,
    /// Name hash from the Stream Extension (for directory re-writes).
    pub name_hash: u16,
    /// Full filename decoded from File Name entries.
    pub name: String,
    /// Decoded creation timestamp.
    pub create_time: ExfatTimestamp,
    /// Decoded modification timestamp.
    pub modified_time: ExfatTimestamp,
    /// Decoded access timestamp.
    pub access_time: ExfatTimestamp,
    /// Byte offset of the File Entry within the image (for writing back).
    pub dir_entry_offset: u64,
    /// Number of secondary entries in the set (for writing back).
    pub secondary_count: u8,
    /// Cluster chain
    pub cluster_chain: Vec<u32>,
}

impl InodeOperations for ExfatInode {
    fn get_file_type(&self) -> FileType {
        self.file_type
    }

    fn set_file_type(&mut self, ft: FileType) {
        // ExFAT doesn't support symlinks, device nodes, or FIFOs.
        self.file_type = match ft {
            FileType::Directory => FileType::Directory,
            _ => FileType::Regular,
        };
    }

    fn get_mode(&self) -> u16 {
        0o755
    }

    fn set_mode(&mut self, _mode: u16) {
        // ExFAT doesn't support Unix permissions, so this is a no-op.
    }

    fn get_atime(&self) -> u64 {
        let timestamp: u64 = self.access_time.into();
        timestamp
    }

    fn set_atime(&mut self, atime: u64) {
        self.access_time = atime.into();
    }

    fn get_mtime(&self) -> u64 {
        let timestamp: u64 = self.modified_time.into();
        timestamp
    }

    fn set_mtime(&mut self, mtime: u64) {
        self.modified_time = mtime.into();
    }

    fn get_ctime(&self) -> u64 {
        let timestamp: u64 = self.create_time.into();
        timestamp
    }

    fn set_ctime(&mut self, ctime: u64) {
        self.create_time = ctime.into();
    }

    fn get_uid(&self) -> u32 {
        0
    }

    fn set_uid(&mut self, _uid: u32) {
        // ExFAT doesn't support Unix UIDs, so this is a no-op.
    }

    fn get_gid(&self) -> u32 {
        0
    }

    fn set_gid(&mut self, _gid: u32) {
        // ExFAT doesn't support Unix GIDs, so this is a no-op.
    }

    fn get_nlinks(&self) -> u32 {
        1_u32
    }

    fn set_nlinks(&mut self, _nlinks: u32) {
        // ExFAT doesn't support hard links, so this is a no-op.
    }

    fn get_size(&self) -> u64 {
        self.valid_data_length
    }

    fn set_size(&mut self, size: u64) {
        self.valid_data_length = size;
    }

    fn get_blocks(&self) -> Vec<u64> {
        self.cluster_chain
            .iter()
            .map(|&cluster| cluster as u64)
            .collect()
    }

    fn set_blocks(&mut self, blocks: &[u64]) {
        self.cluster_chain = blocks.iter().map(|&b| b as u32).collect();
    }

    fn get_node(&self) -> (u16, u16) {
        (0, 0)
    }

    fn set_node(&mut self, _major: u16, _minor: u16) {
        // ExFAT doesn't support device nodes, so this is a no-op.
    }
}
