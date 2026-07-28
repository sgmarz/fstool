//! Bitmap Data Structure
//!
//! Most file systems have bitmaps to keep track of taken/freed
//! and blocks, so have one we can use to quickly find, set, and clear
//! these bitmaps.
//!
//! © Stephen Marz
//! 8 June 2026
use std::io;

#[derive(Default)]
pub struct Bitmap {
    pub map: Vec<u8>,
    pub max: usize,
}

impl Bitmap {
    pub fn take(map: Vec<u8>, max: usize) -> Self {
        Self { map, max }
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_index_ok(&self, index: usize) -> bool {
        let byte_index = index / 8;
        byte_index <= self.len()
    }

    pub fn next(&self) -> Option<usize> {
        for i in 0..self.len() {
            if self.map[i] != 0xFF {
                let end = if 8 * i > self.max {
                    self.max - 8 * i
                }
                else {
                    8
                };
                for j in 0..end {
                    if (self.map[i] & (1 << j)) == 0 {
                        return Some((i * 8) + j);
                    }
                }
            }
        }
        None
    }

    pub fn take_next(&mut self) -> Option<usize> {
        if let Some(index) = self.next() {
            if let Err(_) = self.set(index) {
                return None;
            }
            return Some(index);
        }
        None
    }

    pub fn is_set(&self, index: usize) -> io::Result<bool> {
        let byte_index = index / 8;
        let bit_index = index % 8;
        Ok((self.map[byte_index] & (1 << bit_index)) != 0)
    }

    pub fn set(&mut self, index: usize) -> io::Result<()> {
        let byte_index = index / 8;
        let bit_index = index % 8;
        if !self.is_index_ok(index) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Index out of bounds",
            ));
        }
        self.map[byte_index] |= 1 << bit_index;
        Ok(())
    }

    pub fn clear(&mut self, index: usize) -> io::Result<()> {
        let byte_index = index / 8;
        let bit_index = index % 8;
        if !self.is_index_ok(index) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Index out of bounds",
            ));
        }
        self.map[byte_index] &= !(1 << bit_index);
        Ok(())
    }

    pub fn count_taken(&self) -> usize {
        let mut ret = 0;
        for i in 0..self.map.len() {
            let end = if (8 * i + 7) > self.max {
                self.max - 8 * i
            }
            else {
                8
            };
            for j in 0..end {
                if (self.map[i] & (1 << j)) != 0 {
                    ret += 1;
                }
            }
        }
        ret
    }

    pub fn count_free(&self) -> usize {
        let mut ret = 0;
        for i in 0..self.map.len() {
            let end = if (8 * i + 7) > self.max {
                self.max - 8 * i
            }
            else {
                8
            };
            for j in 0..end {
                if (self.map[i] & (1 << j)) == 0 {
                    ret += 1;
                }
            }
        }
        ret
    }

    pub fn get_map(&self) -> &[u8] {
        &self.map
    }

    pub fn get_map_mut(&mut self) -> &mut [u8] {
        &mut self.map
    }
}
