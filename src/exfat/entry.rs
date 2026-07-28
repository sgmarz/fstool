//! ExFAT Entries
//!
//! © Stephen Marz
//! 8 June 2026
use crate::exfat::consts;

#[repr(C, packed)]
#[derive(Debug)]
/// Points to the cluster that holds the Allocation Bitmap — a bit array with
/// one bit per data cluster indicating free (0) or allocated (1).
///
/// On TexFAT volumes (2 FATs) there are two Allocation Bitmap entries.
/// Bit 0 of `bitmap_flags` distinguishes them: 0 = first bitmap, 1 = second.
pub struct AllocBitmapEntry {
    /// Must be `TYPE_ALLOC_BITMAP` (0x81).
    pub entry_type: u8,
    /// Bit 0: 0 = first allocation bitmap, 1 = second (TexFAT).
    pub bitmap_flags: u8,
    /// Reserved; must be zero.
    pub reserved: [u8; 18],
    /// First cluster of the bitmap.
    pub first_cluster: u32,
    /// Size of the bitmap in bytes.
    /// Must be `ceil(cluster_count / 8)`.
    pub data_length: u64,
}

impl AllocBitmapEntry {
    /// ### Convert a slice into an AllocBitmapEntry
    ///
    /// #### Requirements
    ///
    /// * Length of bytes must be the same size as the structure
    pub fn from_bytes(bytes: &[u8]) -> Self {
        debug_assert!(bytes.len() == consts::ENTRY_SIZE);
        unsafe { std::ptr::read(bytes.as_ptr() as *const AllocBitmapEntry) }
    }

    pub fn get_first_cluster(&self) -> u32 {
        let first_cluster = self.first_cluster;
        first_cluster
    }

    pub fn get_data_length(&self) -> u64 {
        let data_length = self.data_length;
        data_length
    }
}

#[repr(C, packed)]
#[derive(Debug)]
/// Points to the Upcase Table — a 128 KiB array of u16 values mapping every
/// Unicode code point (0x0000–0xFFFF) to its uppercase equivalent.
///
/// Used for case-insensitive filename comparison.  Implementations MUST use
/// this table rather than their own upcase logic so that comparison is
/// consistent across operating systems.
pub struct UpcaseTableEntry {
    /// Must be `TYPE_UPCASE_TABLE` (0x82).
    pub entry_type: u8,
    /// Reserved; must be zero.
    pub reserved1: [u8; 3],
    /// CRC32 of the upcase table data.
    /// The standard table always has checksum `0xE619D30D`.
    pub table_checksum: u32,
    /// Reserved; must be zero.
    pub reserved2: [u8; 12],
    /// Size of the table in bytes (always 131072 = 2 × 65536).
    pub data_length: u64,
    /// First cluster of the table data.
    pub first_cluster: u32,
}

/// Stores the volume label as a UTF-16LE string of up to 11 code units.
/// There is at most one Volume Label entry; its absence means no label is set.
#[repr(C, packed)]
#[derive(Debug)]
pub struct VolumeLabelEntry {
    /// Must be `TYPE_VOLUME_LABEL` (0x83).
    pub entry_type: u8,
    /// Number of UTF-16 code units in `volume_label` (0–11).
    pub character_count: u8,
    /// Volume label in UTF-16LE, padded with zeros.
    pub volume_label: [u16; 11],
    /// Reserved; must be zero.
    pub reserved: [u8; 8],
}

/// The primary directory entry for every file and directory.
///
/// It is always immediately followed by exactly `secondary_count` secondary
/// entries: first a Stream Extension (0xC0), then one or more File Name
/// entries (0xC1).
///
/// The `set_checksum` field is a CRC16 over all entries in the set (including
/// this one), computed with `set_checksum` treated as zero.
#[repr(C, packed)]
#[derive(Debug)]
pub struct FileEntry {
    /// Must be `TYPE_FILE` (0x85).
    pub entry_type: u8,
    /// Number of secondary entries that follow this entry.
    /// Must be 2–18: 1 Stream Extension + 1–17 File Name entries.
    pub secondary_count: u8,
    /// CRC16 over all entries in the set (this + secondary_count entries),
    /// computed with this field set to zero.
    pub set_checksum: u16,
    /// File attribute bits (see `ATTR_*` constants).
    pub file_attributes: u16,
    /// Reserved; must be zero.
    pub reserved1: u16,
    /// File creation timestamp (packed, see `ExfatTimestamp`).
    pub create_time: u32,
    /// Last data modification timestamp.
    pub modified_time: u32,
    /// Last access timestamp.
    pub access_time: u32,
    /// 10ms increment for creation time (0–199).
    pub create_time_10ms: u8,
    /// 10ms increment for modification time (0–199).
    pub modified_time_10ms: u8,
    /// UTC offset for creation time (15-minute increments, signed, −48..+56).
    /// 0x00 means "no timezone info".
    pub create_utc_offset: u8,
    /// UTC offset for modification time.
    pub modified_utc_offset: u8,
    /// UTC offset for access time.
    pub access_utc_offset: u8,
    /// Reserved; must be zero.
    pub reserved2: [u8; 7],
}

/// The first secondary entry in every file entry set.  It carries the file's
/// size, first cluster, and the `NoFatChain` optimization flag.
///
/// `valid_data_length` ≤ `data_length`.  `data_length` is always a multiple
/// of the cluster size; `valid_data_length` is the number of bytes the
/// application has actually written.  The difference is zero-filled on read.
#[repr(C, packed)]
#[derive(Debug)]
pub struct StreamExtensionEntry {
    /// Must be `TYPE_STREAM_EXTENSION` (0xC0).
    pub entry_type: u8,
    /// General secondary flags (see `GEN_FLAG_*` constants).
    ///
    /// `GEN_FLAG_NO_FAT_CHAIN` (bit 1): when set, clusters are contiguous
    /// and the FAT need not be followed.
    pub general_secondary_flags: u8,
    /// Reserved; must be zero.
    pub reserved1: u8,
    /// Length of the filename in UTF-16 code units (1–255).
    pub name_length: u8,
    /// Hash of the upcased filename, used for fast directory search.
    /// Allows skipping File Name entry reads when the hash doesn't match.
    pub name_hash: u16,
    /// Reserved; must be zero.
    pub reserved2: u16,
    /// Number of bytes of valid (written) data.
    /// Always ≤ `data_length`.
    pub valid_data_length: u64,
    /// Reserved; must be zero.
    pub reserved3: u32,
    /// First cluster of the file's data.  0 if the file is empty.
    pub first_cluster: u32,
    /// Allocated data size in bytes.  Always a multiple of cluster size.
    pub data_length: u64,
}

/// One or more File Name secondary entries follow the Stream Extension.
/// Each holds up to 15 UTF-16 code units of the filename.
///
/// Entries appear in order (first entry holds chars 0–14, second holds 15–29,
/// etc.).  Unlike FAT32 LFN entries, there is no reverse ordering — the first
/// File Name entry on disk is the first logical chunk of the name.
#[repr(C, packed)]
#[derive(Debug)]
pub struct FileNameEntry {
    /// Must be `TYPE_FILE_NAME` (0xC1).
    pub entry_type: u8,
    /// General secondary flags.  Must be 0 (no `NoFatChain` for name entries).
    pub general_secondary_flags: u8,
    /// Up to 15 UTF-16LE code units of the filename.  Unused slots are zero.
    pub file_name: [u16; 15],
}

/// Optional entry stored in the root directory to uniquely identify the volume.
/// GUIDs follow the standard 16-byte mixed-endian layout (RFC 4122).
#[repr(C, packed)]
#[derive(Debug)]
pub struct VolumeGuidEntry {
    /// Must be `TYPE_VOLUME_GUID` (0xA0).
    pub entry_type: u8,
    /// Must be 0.
    pub secondary_count: u8,
    /// CRC16 of this entry (with this field set to zero during computation).
    pub set_checksum: u16,
    /// General primary flags; must be 0.
    pub general_primary_flags: u16,
    /// The 16-byte GUID in RFC 4122 mixed-endian layout.
    pub volume_guid: [u8; 16],
    /// Reserved; must be zero.
    pub reserved: [u8; 10],
}
