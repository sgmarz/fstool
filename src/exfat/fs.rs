//! ExFAT File System Implementation
//!
//! © Stephen Marz
//! 8 June 2026
use super::ExfatFileSystem;
use crate::cache::Tree;
use crate::filetype::FileType;
use crate::fs::{FileSystem, InodeOperations, SuperblockOperations};
use std::io::{self, Read, Seek, SeekFrom};

impl FileSystem for ExfatFileSystem {
    fn name(&self) -> &str {
        "exFAT"
    }

    /// ### Read a Cluster
    ///
    /// Even though the name of this is read_block, ExFAT uses
    /// clusters instead. A cluster is a multiple of a sector size.
    fn read_block(&mut self, cluster_num: u64, data: &mut [u8]) -> io::Result<()> {
        let size = usize::min(data.len(), self.mbs.get_block_size() as usize);
        let byte_offset = self.mbs.cluster_byte_offset(cluster_num as u32);
        if self.bcache.copy_block_to(cluster_num, &mut data[..size]) {
            return Ok(());
        }
        // Data was not found in cache, read it from the disk.
        let mut cluster = vec![0_u8; size];
        self.stream.seek(SeekFrom::Start(byte_offset))?;
        self.stream.read_exact(&mut cluster)?;
        data[..size].copy_from_slice(&cluster[..size]);
        self.bcache.insert_read(cluster_num, cluster.to_vec());
        Ok(())
    }

    fn write_block(&mut self, cluster_num: u64, data: &[u8]) -> io::Result<()> {
        let size = usize::min(self.mbs.get_block_size() as usize, data.len());
        if !self.bcache.insert_write(cluster_num, data[..size].to_vec()) {
            return Err(io::Error::new(
                io::ErrorKind::OutOfMemory,
                "block cache insert failed",
            ));
        }
        Ok(())
    }

    fn get_inode(&mut self, _inode_num: u64) -> io::Result<&dyn InodeOperations> {
        todo!();
    }

    fn get_inode_mut(&mut self, _inode_num: u64) -> io::Result<&mut dyn InodeOperations> {
        todo!();
    }

    fn create(&mut self, abs_path: &String, ftype: FileType) -> io::Result<u64> {
        match ftype {
            FileType::Directory | FileType::Regular => {}
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "Unsupported file type",
                ));
            }
        }
        todo!();
    }

    fn link(&mut self, _parent_inode: u64, _abs_path: &String) -> io::Result<u64> {
        Err(io::Error::new(
            io::ErrorKind::Other,
            "ExFAT does not support hard links",
        ))
    }

    fn read_symlink(&mut self, inode: u64) -> io::Result<String> {
        Err(io::Error::new(
            io::ErrorKind::Other,
            "ExFAT does not support symlinks",
        ))
    }

    fn write_symlink(&mut self, _inode: u64, _target: &String) -> io::Result<u64> {
        Err(io::Error::new(
            io::ErrorKind::Other,
            "ExFAT does not support symlinks",
        ))
    }

    fn read_file(&mut self, inode: u64, offset: u64, buffer: &mut [u8]) -> io::Result<u64> {
        todo!();
    }

    fn write_file(&mut self, inode: u64, offset: u64, buffer: &[u8]) -> io::Result<u64> {
        todo!();
    }

    fn truncate(&mut self, inode: u64, size: u64) -> io::Result<u64> {
        todo!();
    }

    fn unlink(&mut self, abs_path: &String) -> io::Result<()> {
        todo!();
    }

    fn write_to_backing(&mut self) -> io::Result<()> {
        todo!();
    }

    fn get_superblock(&self) -> &dyn SuperblockOperations {
        &self.mbs
    }

    fn get_superblock_mut(&mut self) -> &mut dyn SuperblockOperations {
        &mut self.mbs
    }

    fn get_tree(&self) -> Option<&Tree> {
        todo!();
    }

    fn get_tree_mut(&mut self) -> Option<&mut Tree> {
        todo!();
    }
}
