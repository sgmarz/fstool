//! Generic File Types
//!
//! © Stephen Marz
//! 8 June 2026
use super::stat;
use std::string::ToString;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum FileType {
    Invalid,
    Regular,
    Directory,
    Symlink,
    BlockDevice,
    CharacterDevice,
    Fifo,
    Socket,
}

impl FileType {
    pub fn sort_order(&self) -> usize {
        match self {
            FileType::Directory => 0,
            FileType::Symlink => 1,
            FileType::Regular => 3,
            _ => 2,
        }
    }
}

impl Into<u16> for FileType {
    fn into(self) -> u16 {
        match self {
            FileType::Regular => stat::S_IFREG,
            FileType::Directory => stat::S_IFDIR,
            FileType::Symlink => stat::S_IFLNK,
            FileType::BlockDevice => stat::S_IFBLK,
            FileType::CharacterDevice => stat::S_IFCHR,
            FileType::Fifo => stat::S_IFIFO,
            FileType::Socket => stat::S_IFSOCK,
            FileType::Invalid => 0,
        }
    }
}

impl Into<FileType> for u16 {
    fn into(self) -> FileType {
        match self & stat::S_IFMT {
            stat::S_IFREG => FileType::Regular,
            stat::S_IFDIR => FileType::Directory,
            stat::S_IFLNK => FileType::Symlink,
            stat::S_IFBLK => FileType::BlockDevice,
            stat::S_IFCHR => FileType::CharacterDevice,
            stat::S_IFIFO => FileType::Fifo,
            stat::S_IFSOCK => FileType::Socket,
            _ => FileType::Invalid,
        }
    }
}

impl ToString for FileType {
    fn to_string(&self) -> String {
        match self {
            FileType::Regular => "Regular".to_string(),
            FileType::Directory => "Directory".to_string(),
            FileType::Symlink => "Symlink".to_string(),
            FileType::BlockDevice => "Block Device".to_string(),
            FileType::CharacterDevice => "Character Device".to_string(),
            FileType::Fifo => "FIFO".to_string(),
            FileType::Socket => "Socket".to_string(),
            FileType::Invalid => "Invalid".to_string(),
        }
    }
}
