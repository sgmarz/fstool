//! ExFAT Constants
//!
//! © Stephen Marz
//! 8 June 2026
#![allow(dead_code)]
pub const MINIMUM_IMAGE_SIZE: u64 = 4096 * 512 * 2;

// MAIN BOOT SECTOR
/// The OEM name field always contains this value for exFAT volumes.
pub const EXFAT_OEM_NAME: &[u8; 8] = b"EXFAT   ";

pub const BOOT_JUMP_CODE: [u8; 3] = [0xEB, 0x76, 0x90];

/// Boot sector signature at bytes 510–511 (same as FAT32).
pub const BOOT_SIGNATURE: u16 = 0xAA55;
pub const EXTENDED_BOOT_SIGNATURE: u32 = 0xAA550000;

/// Size of the main boot sector, in bytes (must be 512 for compatibility with FAT32).
pub const MAIN_BOOT_SECTOR_SIZE: usize = 512;
/// Size of the main boot sector as a shift (1 << 9 = 512).
pub const MAIN_SECTOR_SHIFT: usize = 9;

// CLUSTERS
/// Clusters 0 and 1 are reserved; real data starts at cluster 2.
pub const FIRST_DATA_CLUSTER: u32 = 2;

/// A free cluster in the FAT.
pub const EXFAT_FREE: u32 = 0x0000_0000;

/// A bad (permanently unusable) cluster.
pub const EXFAT_BAD: u32 = 0xFFFF_FFF7;

/// A cluster value ≥ this marks end-of-chain.
pub const EXFAT_EOC_MIN: u32 = 0xFFFF_FFF8;

/// A media cluster
pub const EXFAT_MEDIA: u32 = 0xFFFF_FFF8;

/// The canonical end-of-chain marker written by most implementations.
pub const EXFAT_EOC: u32 = 0xFFFF_FFFF;

// ENTRY ATTRIBUTES
pub const ATTR_READ_ONLY: u16 = 0x0001;
pub const ATTR_HIDDEN: u16 = 0x0002;
pub const ATTR_SYSTEM: u16 = 0x0004;
pub const ATTR_DIRECTORY: u16 = 0x0010;
pub const ATTR_ARCHIVE: u16 = 0x0020;

// ENTRY TYPES
// The high bit (0x80) indicates the entry is "in use".  Clearing it marks the
// entry as deleted without erasing its contents (so recovery tools can find it).

pub const TYPE_END_OF_DIR: u8 = 0x00; // stop scanning
pub const TYPE_ALLOC_BITMAP: u8 = 0x81; // allocation bitmap file
pub const TYPE_UPCASE_TABLE: u8 = 0x82; // Unicode upcase table file
pub const TYPE_VOLUME_LABEL: u8 = 0x83; // volume label
pub const TYPE_FILE: u8 = 0x85; // primary file / directory entry
pub const TYPE_VOLUME_GUID: u8 = 0xA0; // optional volume GUID
pub const TYPE_TEXFAT_PADDING: u8 = 0xA1; // TexFAT padding (2-FAT volumes)
pub const TYPE_STREAM_EXTENSION: u8 = 0xC0; // secondary: size + first cluster
pub const TYPE_FILE_NAME: u8 = 0xC1; // secondary: UTF-16 name fragment

/// Bit set in an entry type byte when the entry is "in use".
pub const ENTRY_IN_USE_BIT: u8 = 0x80;
pub const ENTRY_SIZE: usize = 32;

// GENERAL SECONDARY FLAGS
/// Always 1 for real files; 0 only for the Upcase Table (which is pre-built).
pub const GEN_FLAG_ALLOC_POSSIBLE: u8 = 0x01;

/// When set, the file's clusters are *contiguous* and the FAT chain need not
/// be followed — the extent is fully described by first_cluster + data_length.
/// This is ExFAT's major performance optimization for large sequential writes.
pub const GEN_FLAG_NO_FAT_CHAIN: u8 = 0x02;


// TIMESTAMP ENCODING
/// The year offset used in packed ExFAT timestamps (year field = actual_year − 1980).
pub const EXFAT_EPOCH_YEAR: u16 = 1980;

/// Maximum value of the 10ms-increment sub-second field (0–199 = 0.00s–1.99s).
pub const EXFAT_10MS_MAX: u8 = 199;

/// UTC offset field meaning "no timezone information".
pub const EXFAT_UTC_UNKNOWN: u8 = 0x00;


// UPCASE TABLE
/// CRC32 of the standard exFAT upcase table (all 2^16 entries).
/// Used to verify the table after reading it from disk.
pub const UPCASE_TABLE_CHECKSUM: u32 = 0xE619_D30D;

/// Number of UTF-16 code units in the full upcase table.
pub const UPCASE_TABLE_CHARS: usize = 65536;


// NAME LIMITS
/// Maximum filename length in UTF-16 code units.
pub const MAX_NAME_LENGTH: usize = 255;

/// UTF-16 code units stored per File Name directory entry.
pub const NAME_CHARS_PER_ENTRY: usize = 15;

/// Maximum number of File Name entries in a set (ceil(255 / 15)).
pub const MAX_FILE_NAME_ENTRIES: usize = 17;

/// Maximum number of secondary entries in a single file entry set.
/// 1 Stream Extension + up to 17 File Name entries.
pub const MAX_SECONDARY_COUNT: u8 = 18;


// VOLUME LABEL
/// Maximum volume label length in UTF-16 code units.
pub const MAX_VOLUME_LABEL_LENGTH: usize = 11;

/// Boot code
pub const BOOTCODE: [u8; 29] = [
    0x0e, // push cs
    0x1f, // pop ds
    0xbe, 0x00, 0x68, // mov si, offset message_txt (to be filled later)
    // write_msg:
    0xac, // lodsb
    0x22, 0xc0, // and al, al
    0x74, 0x0b, // jz key_press
    0x56, // push si
    0xb4, 0x0e, // mov ah, 0eh
    0xbb, 0x07, 0x00, // mov bx, 0007h
    0xcd, 0x10, // int 10h
    0x5e, // pop si
    0xeb, 0xf0, // jmp write_msg
    // key_press:
    0x32, 0xe4, // xor ah, ah
    0xcd, 0x16, // int 16h
    0xcd, 0x19, // int 19h
    0xeb, 0xfe, // foo: jmp foo
];

pub const BOOTCODE_MSG: &str = "This exFAT/GPT volume is not bootable.\r\nPlease insert a bootable disk and press any key to try again.\r\n";
