//! In-memory Caching Tree
//!
//! © Stephen Marz
//! 8 June 2026
use crate::filetype::FileType;

#[derive(Debug, Clone)]
pub enum Item {
    File(ItemData),
    Directory(ItemData, Tree),
    Symlink(ItemData, String), // target path
}

impl Item {
    pub const fn new(name: String, inode_num: u64, ftype: FileType) -> Self {
        match ftype {
            FileType::Directory => Item::new_dir(name, inode_num, Vec::new()),
            FileType::Symlink => Item::new_symlink(name, inode_num, String::new()),
            _ => Item::new_file(name, inode_num),
        }
    }

    pub const fn new_file(name: String, inode_num: u64) -> Self {
        Item::File(ItemData { name, inode_num })
    }

    pub const fn new_dir(name: String, inode_num: u64, children: Tree) -> Self {
        Item::Directory(ItemData { name, inode_num }, children)
    }

    pub const fn new_symlink(name: String, inode_num: u64, target: String) -> Self {
        Item::Symlink(ItemData { name, inode_num }, target)
    }

    pub fn is_dir(&self) -> bool {
        matches!(self, Item::Directory(_, _))
    }

    pub fn is_symlink(&self) -> bool {
        matches!(self, Item::Symlink(_, _))
    }

    pub fn symlink_target(&self) -> Option<String> {
        match self {
            Item::Symlink(_, target) => Some(target.clone()),
            _ => None,
        }
    }

    pub fn inode(&self) -> u64 {
        match self {
            Item::File(data) => data.inode_num,
            Item::Directory(data, _) => data.inode_num,
            Item::Symlink(data, _) => data.inode_num,
        }
    }

    pub fn name(&self) -> &String {
        match self {
            Item::File(data) => &data.name,
            Item::Directory(data, _) => &data.name,
            Item::Symlink(data, _) => &data.name,
        }
    }

    pub fn clone_name(&self) -> String {
        self.name().clone()
    }

    pub fn next(&self) -> Option<&Tree> {
        match self {
            Item::File(_) => None,
            Item::Directory(_, children) => Some(children),
            Item::Symlink(_, _) => None,
        }
    }

    pub fn next_mut(&mut self) -> Option<&mut Tree> {
        match self {
            Item::File(_) => None,
            Item::Directory(_, children) => Some(children),
            Item::Symlink(_, _) => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ItemData {
    pub name: String,
    pub inode_num: u64,
}

pub type Tree = Vec<Item>;

use std::collections::HashMap;

use crate::minix::consts;

// Policy for non-dirty block eviction. This is only used
// when the cache is full and we need to insert a new block.
pub enum CachePolicy {
    Lfu,  // Least Frequently Used
    Lru,  // Least Recently Used
    Fifo, // First In, First Out
    Stop, // Stop caching when full, return error on insert
}

pub struct BlockCache {
    cache: HashMap<u64, CachedBlock>,
    private: usize,
    policy: CachePolicy,
}
impl Default for BlockCache {
    fn default() -> Self {
        Self::new(CachePolicy::Lru)
    }
}
impl BlockCache {
    pub fn new(policy: CachePolicy) -> Self {
        Self {
            cache: HashMap::new(),
            private: 0,
            policy,
        }
    }

    pub fn clear(&mut self) {
        self.cache.clear();
    }

    // pub fn contains(&self, bnum: u64) -> bool {
    //     self.cache.contains_key(&bnum)
    // }

    pub fn copy_block_to(&self, bnum: u64, data: &mut [u8]) -> bool {
        if let Some(source) = self.get(bnum) {
            let size = usize::min(source.data.len(), data.len());
            data[..size].copy_from_slice(&source.data[..size]);
            true
        }
        else {
            false
        }
    }

    // pub fn is_empty(&self) -> bool {
    //     self.cache.is_empty()
    // }

    pub fn is_full(&self) -> bool {
        self.cache.len() >= consts::CACHE_LINES
    }

    // pub fn for_each<F>(&self, f: F)
    // where
    //     F: FnMut((&u64, &CachedBlock)),
    // {
    //     self.cache.iter().for_each(f);
    // }

    // pub fn for_each_clean<F>(&self, f: F)
    // where
    //     F: FnMut((&u64, &CachedBlock)),
    // {
    //     self.cache.iter().filter(|(_, x)| !x.dirty).for_each(f);
    // }

    pub fn for_each_dirty<F>(&self, f: F)
    where
        F: FnMut((&u64, &CachedBlock)),
    {
        self.cache.iter().filter(|(_, x)| x.dirty).for_each(f);
    }

    pub fn clean_all(&mut self) {
        self.cache.iter_mut().for_each(|(_, x)| x.dirty = false);
    }

    pub fn get(&self, bnum: u64) -> Option<&CachedBlock> {
        self.cache.get(&bnum)
    }

    // pub fn get_mut(&mut self, bnum: u64) -> Option<&mut CachedBlock> {
    //     self.cache.get_mut(&bnum)
    // }

    pub fn insert(&mut self, bnum: u64, data: Vec<u8>, dirty: bool, overfill: bool) -> bool {
        if self.is_full() && !overfill {
            return false;
        }
        let c = CachedBlock { dirty, data };
        self.cache.insert(bnum, c);
        true
    }

    pub fn insert_write(&mut self, bnum: u64, data: Vec<u8>) -> bool {
        self.insert(bnum, data, true, true)
    }

    pub fn insert_read(&mut self, bnum: u64, data: Vec<u8>) -> bool {
        if self.is_full() {
            // If the cache is full, we need to evict a non-dirty block before
            // inserting the new block.
        }
        self.insert(bnum, data, false, false)
    }

    // pub fn len(&self) -> usize {
    //     self.cache.len()
    // }

    // pub fn remove(&mut self, bnum: u64) {
    //     let _ = self.cache.remove(&bnum);
    // }
}

pub struct CachedBlock {
    pub dirty: bool,
    pub data: Vec<u8>,
}
