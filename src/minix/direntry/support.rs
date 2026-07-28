//! Minix 3 File System Directory Entry Implementation
//!
//! © Stephen Marz
//! 8 June 2026
use super::super::consts::DIR_ENTRY_NAME_SIZE;
use super::DirEntry;

impl DirEntry {
    pub fn new(inode: u32, name: &str) -> Self {
        let mut name_bytes = [0; DIR_ENTRY_NAME_SIZE];
        let name_bytes_truncated = &name.as_bytes()[..DIR_ENTRY_NAME_SIZE.min(name.len())];
        name_bytes[..name_bytes_truncated.len()].copy_from_slice(name_bytes_truncated);
        Self {
            inode,
            name: name_bytes,
        }
    }

    pub fn from_bytes_mut<'a>(bytes: &'a mut [u8]) -> Option<&'a mut Self> {
        assert!(bytes.len() as usize >= size_of::<Self>());
        unsafe {
            let ptr = bytes.as_mut_ptr() as *mut Self;
            ptr.as_mut()
        }
    }

    pub fn name(&self) -> String {
        let cleaned_name = self
            .name
            .iter()
            .take_while(|&&b| b != 0)
            .cloned()
            .collect::<Vec<u8>>();
        std::str::from_utf8(&cleaned_name)
            .unwrap_or("<invalid utf-8>")
            .to_string()
    }
}
