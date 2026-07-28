//! exFAT Main Boot Sector (MBS) structure and related constants.
//!
//! © Stephen Marz
//! 8 June 2026
use super::consts;
use crate::fs::{AllocationData, SuperblockOperations};
use std::{cmp::Ordering, io, ptr, slice::from_raw_parts};

/// The main boot sector occupies the first 512 bytes of the volume.
/// It is followed by 11 extended boot sectors, an OEM parameters sector,
/// a reserved sector, and a checksum sector — 12 sectors total in the
/// main boot region.  The backup boot region is an identical copy at
/// sector 12.
///
/// Notable differences from FAT32's BPB:
///   • `bytes_per_sector_shift` and `sectors_per_cluster_shift` store log₂
///     of the actual values rather than the values themselves.
///   • All size/offset fields that were 32-bit in FAT32 are now 64-bit.
///   • 53 bytes at offset 11 are explicitly reserved and must be zero.
///   • A separate boot checksum sector (last in the boot region) holds a
///     CRC32 over sectors 0–10 for integrity verification.
#[repr(C, packed)]
pub struct MainBootSector {
    /// x86 jump-to-boot-code instruction (0xEB 0x76 0x90 typically).
    pub jump_boot: [u8; 3],
    /// Must be "EXFAT   " (8 bytes, 3 trailing spaces).
    pub oem_name: [u8; 8],
    /// Must be zero (was the BPB in FAT; exFAT leaves it zeroed to prevent
    /// FAT drivers from misidentifying the volume).
    pub must_be_zero: [u8; 53],
    /// Sector offset of this partition within the physical drive.
    pub partition_offset: u64,
    /// Total number of sectors in the volume.
    pub volume_length: u64,
    /// Sector offset of the first FAT from the start of the partition.
    pub fat_offset: u32,
    /// Number of sectors occupied by each FAT.
    pub fat_length: u32,
    /// Sector offset of the cluster heap (data region) from partition start.
    pub cluster_heap_offset: u32,
    /// Total number of data clusters in the cluster heap.
    pub cluster_count: u32,
    /// Cluster number of the first cluster of the root directory.
    pub root_directory_cluster: u32,
    /// Volume serial number (generated at format time).
    pub volume_serial_number: u32,
    /// File system revision: high byte = major, low byte = minor.
    /// Current version is 1.00 (0x0100).
    pub file_system_revision: u16,
    /// Volume flags (see `VolumeFlags`).
    pub volume_flags: u16,
    /// log of bytes per sector.  Valid range: 9–12 (512–4096 bytes).
    pub bytes_per_sector_shift: u8,
    /// log of sectors per cluster.  Valid range: 0–25.
    /// Maximum cluster size is 32 MiB (2^25 sectors × 512 bytes).
    pub sectors_per_cluster_shift: u8,
    /// Number of FATs: 1 (normal) or 2 (TexFAT, transaction-safe).
    pub number_of_fats: u8,
    /// INT 13h drive select value (0x00 = floppy, 0x80 = hard disk).
    pub drive_select: u8,
    /// Percentage of the cluster heap that is allocated (0–100).
    /// 0xFF means the value has not been calculated.
    pub percent_in_use: u8,
    /// Reserved; must be zero.
    pub reserved: [u8; 7],
    /// Boot code (jumps over the BPB and bootstraps the OS).
    pub boot_code: [u8; 390],
    /// Boot signature: must be 0xAA55.
    pub boot_signature: u16,
}

impl SuperblockOperations for MainBootSector {
    /// ### Get the smallest transactionable unit.
    ///
    /// ExFAT doesn't have "blocks" per se, but a "cluster" means
    /// essentially the same thing. It is a locatable (addressable) unit.
    ///
    /// The file allocation table refers to the cluster number, and a cluster
    /// is made up one or more sectors.
    fn get_block_size(&self) -> u64 {
        let bps = 1_u64 << self.bytes_per_sector_shift;
        let spc = 1_u64 << self.sectors_per_cluster_shift;
        // The MBS uses "shifts" to calculate the bytes and sectors.
        bps * spc
    }

    fn get_num_blocks(&self) -> AllocationData {
        let taken = if self.percent_in_use == 0xFF {
            0
        }
        else {
            (self.cluster_count as f64 * (self.percent_in_use as f64 / 100.0)).round() as u64
        };
        AllocationData {
            taken,
            free: self.cluster_count as u64 - taken,
        }
    }

    fn get_num_inodes(&self) -> AllocationData {
        AllocationData {
            taken: 0,
            free: u64::MAX, // ExFAT doesn't have a fixed inode limit
        }
    }

    fn allocate_inode(&mut self) -> io::Result<u64> {
        // We should return a directory entry here instead.
        todo!();
    }

    fn deallocate_inode(&mut self, _inode_num: u64) -> io::Result<()> {
        // We need to remove the directory entry.
        todo!();
    }

    fn allocate_block(&mut self) -> io::Result<u64> {
        // We need to look in the fat for any EOC_MIN values. Those are not taken that
        // we can allocate.
        todo!();
    }

    fn deallocate_block(&mut self, _block_num: u64) -> io::Result<()> {
        // We need to set the FAT to EOC_MIN.
        todo!();
    }
}

impl MainBootSector {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        // SAFETY: The caller must ensure that the input slice is exactly 512 bytes long.
        debug_assert!(
            bytes.len() == consts::MAIN_BOOT_SECTOR_SIZE,
            "MainBootSector must be exactly {} bytes",
            consts::MAIN_BOOT_SECTOR_SIZE
        );
        unsafe { ptr::read(bytes.as_ptr() as *const MainBootSector) }
    }

    pub fn to_bytes(&self) -> &[u8] {
        unsafe {
            from_raw_parts(
                self as *const MainBootSector as *const u8,
                std::mem::size_of::<MainBootSector>(),
            )
        }
    }

    /// Checks if the main boot sector is valid by verifying the OEM name and boot signature.
    pub fn is_valid(&self) -> bool {
        self.oem_name.cmp(&consts::EXFAT_OEM_NAME) == Ordering::Equal
            && self.boot_signature == consts::BOOT_SIGNATURE
    }

    pub fn sectors_to_bytes(&self, num: u64) -> u64 {
        num * (1 << self.bytes_per_sector_shift)
    }

    pub fn clusters_to_bytes(&self, num: u64) -> u64 {
        self.sectors_to_bytes(num * (1 << self.sectors_per_cluster_shift))
    }

    pub fn cluster_byte_offset(&self, cluster_num: u32) -> u64 {
        self.sectors_to_bytes(self.cluster_heap_offset as u64)
            + self.clusters_to_bytes(cluster_num.saturating_sub(2) as u64)
    }
}

// DEFAULT implementation for MainBootSector

impl Default for MainBootSector {
    fn default() -> Self {
        let mut ret = Self {
            jump_boot: consts::BOOT_JUMP_CODE, // Typical x86 jump instruction
            oem_name: *consts::EXFAT_OEM_NAME,
            must_be_zero: [0; 53],
            partition_offset: 0,
            volume_length: 0,
            fat_offset: 0,
            fat_length: 0,
            cluster_heap_offset: 0,
            cluster_count: 0,
            root_directory_cluster: 0,
            volume_serial_number: 0,
            file_system_revision: 0x0100, // Version 1.00
            volume_flags: 0,
            bytes_per_sector_shift: 9,    // Default to 512 bytes/sector
            sectors_per_cluster_shift: 3, // Default to 3 sectors/cluster
            number_of_fats: 1,
            drive_select: 0x80, // Default to hard disk
            percent_in_use: 0,
            reserved: [0; 7],
            boot_code: [0; 390], // Filled in below
            boot_signature: consts::BOOT_SIGNATURE,
        };
        // Boot code (display-only; this tool does not boot from ExFAT images).
        for (i, &c) in consts::BOOTCODE.iter().enumerate() {
            ret.boot_code[i] = c;
        }
        for (i, c) in consts::BOOTCODE_MSG.bytes().enumerate() {
            ret.boot_code[i + consts::BOOTCODE.len()] = c;
        }
        ret
    }
}

// DEBUG Implementation for MainBootSector.

pub const GREEN_CHECKMARK: char = '✅';
pub const RED_CROSS: char = '❌';

impl std::fmt::Debug for MainBootSector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Unaligned values
        let bps = 1 << self.bytes_per_sector_shift;
        let spc = 1 << self.sectors_per_cluster_shift;
        let partition_offset = u64::from_le_bytes(self.partition_offset.to_le_bytes());
        let volume_length = u64::from_le_bytes(self.volume_length.to_le_bytes());
        let fat_offset = u32::from_le_bytes(self.fat_offset.to_le_bytes());
        let fat_length = u32::from_le_bytes(self.fat_length.to_le_bytes());
        let cluster_heap_offset = u32::from_le_bytes(self.cluster_heap_offset.to_le_bytes());
        let cluster_count = u32::from_le_bytes(self.cluster_count.to_le_bytes());
        let root_directory_cluster = u32::from_le_bytes(self.root_directory_cluster.to_le_bytes());
        let volume_serial_number = u32::from_le_bytes(self.volume_serial_number.to_le_bytes());
        let file_system_revision = u16::from_le_bytes(self.file_system_revision.to_le_bytes());
        let volume_flags = u16::from_le_bytes(self.volume_flags.to_le_bytes());
        let boot_signature = u16::from_le_bytes(self.boot_signature.to_le_bytes());
        let magic_good = if boot_signature == consts::BOOT_SIGNATURE {
            GREEN_CHECKMARK
        }
        else {
            RED_CROSS
        };
        let oem_good = if self.oem_name.cmp(&consts::EXFAT_OEM_NAME) == Ordering::Equal {
            GREEN_CHECKMARK
        }
        else {
            RED_CROSS
        };
        #[cfg(debug_assertions)]
        {
            writeln!(f, "MainBootSector {{")?;
            writeln!(
                f,
                "    jump_boot: {:02x} {:02x} {:02x},",
                self.jump_boot[0], self.jump_boot[1], self.jump_boot[2]
            )?;
            writeln!(
                f,
                "    oem_name: \"{}\" {},",
                String::from_utf8_lossy(&self.oem_name),
                oem_good
            )?;
            writeln!(f, "    partition_offset: {},", partition_offset)?;
            writeln!(f, "    volume_length: {},", volume_length)?;
            writeln!(f, "    fat_offset: {},", fat_offset)?;
            writeln!(
                f,
                "    fat_length: {} sectors / {} bytes,",
                fat_length,
                fat_length * bps
            )?;
            writeln!(f, "    cluster_heap_offset: {},", cluster_heap_offset)?;
            writeln!(f, "    cluster_count: {},", cluster_count)?;
            writeln!(f, "    root_directory_cluster: {},", root_directory_cluster)?;
            writeln!(
                f,
                "    volume_serial_number: {:04X}-{:04X},",
                (volume_serial_number >> 16) as u16,
                (volume_serial_number & 0xFFFF) as u16
            )?;
            writeln!(
                f,
                "    file_system_revision: {:#06X},",
                file_system_revision
            )?;
            writeln!(f, "    volume_flags: {:#06X},", volume_flags)?;
            writeln!(
                f,
                "    bytes_per_sector_shift: {} ({} bytes),",
                self.bytes_per_sector_shift, bps
            )?;
            writeln!(
                f,
                "    sectors_per_cluster_shift: {} ({} sectors / {} bytes),",
                self.sectors_per_cluster_shift,
                spc,
                spc * bps
            )?;
            writeln!(f, "    number_of_fats: {},", self.number_of_fats)?;
            writeln!(f, "    drive_select: {:#04X},", self.drive_select)?;
            writeln!(f, "    percent_in_use: {},", self.percent_in_use)?;
            writeln!(
                f,
                "    boot_signature: {:#06X} {},",
                boot_signature, magic_good
            )?;
            writeln!(f, "}}")
        }
        #[cfg(not(debug_assertions))]
        {
            f.debug_struct("MainBootSector")
            .field("jump_boot", &format_args!("{:02x} {:02x} {:02x}", self.jump_boot[0], self.jump_boot[1], self.jump_boot[2]))
            .field("oem_name", &format_args!("\"{}\" {}", String::from_utf8_lossy(&self.oem_name), oem_good))
            .field("partition_offset", &partition_offset)
            .field("volume_length", &volume_length)
            .field("fat_offset", &fat_offset)
            .field("fat_length", &format_args!("{} sectors / {} bytes", fat_length, fat_length * bps))
            .field("cluster_heap_offset", &cluster_heap_offset)
            .field("cluster_count", &cluster_count)
            .field("root_directory_cluster", &root_directory_cluster)
            .field("volume_serial_number", &format_args!("{:04X}-{:04X}", (volume_serial_number >> 16) as u16, (volume_serial_number & 0xFFFF) as u16))
            .field("file_system_revision", &format_args!("{:#06X}", file_system_revision))
            .field("volume_flags", &format_args!("{:#06X}", volume_flags))
            .field("bytes_per_sector_shift", &format_args!("{} ({} bytes)", self.bytes_per_sector_shift, bps))
            .field("sectors_per_cluster_shift", &format_args!("{} ({} sectors / {} bytes)", self.sectors_per_cluster_shift, spc, spc * bps))
            .field("number_of_fats", &self.number_of_fats)
            .field("drive_select", &format_args!("{:#04X}", self.drive_select))
            .field("percent_in_use", &self.percent_in_use)
            .field("boot_signature", &format_args!("{:#06X} {}", boot_signature, magic_good))
            // Omit reserved and boot code for brevity
            .finish()
        }
    }
}
