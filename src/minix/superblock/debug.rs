//! Minix 3 File System Superblock Debugging Output
//! Useful when debugging/printing the superblock.
//!
//! © Stephen Marz
//! 8 June 2026
use super::Superblock;
use std::fmt::{Formatter, Result};

pub const GREEN_CHECKMARK: char = '✅';
pub const RED_CROSS: char = '❌';

impl std::fmt::Debug for Superblock {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.debug_struct("Superblock")
            .field("num_inodes", &self.num_inodes)
            .field("imap_blocks", &self.imap_blocks)
            .field("zmap_blocks", &self.zmap_blocks)
            .field("first_data_zone", &self.first_data_zone)
            .field("log_zone_size", &self.log_zone_size)
            .field("max_size", &self.max_size)
            .field("num_zones", &self.num_zones)
            .field(
                "magic",
                &format_args!(
                    "{:#06x} {}",
                    self.magic,
                    if self.is_valid() {
                        GREEN_CHECKMARK
                    }
                    else {
                        RED_CROSS
                    }
                ),
            )
            .field("block_size", &self.block_size)
            .field("disk_version", &self.disk_version)
            .finish()
    }
}
