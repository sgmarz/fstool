//! ExFAT Make File System.
//!
//! © Stephen Marz
//! 8 June 2026
use super::{ExfatFileSystem, consts, mbs::MainBootSector};
use crate::{align_up, fs::MakeFileSystem};
use chrono::Local;
use std::{
    fs::File,
    io::{self, Seek, SeekFrom, Write},
};

// -- Boot-region checksum (spec 3.4) --------------------------------------

/// Compute the ExFAT boot-region checksum over `sectors_0_to_10` (5 632 bytes).
///
/// Three byte positions are excluded because they change during normal
/// operation and must not invalidate the checksum:
///   - 106 and 107  `VolumeFlags`
///   - 112          `PercentInUse`
fn boot_checksum(sectors_0_to_10: &[u8]) -> u32 {
    debug_assert_eq!(sectors_0_to_10.len(), 512 * 11);
    let mut csum: u32 = 0;
    for (i, &b) in sectors_0_to_10.iter().enumerate() {
        if i == 106 || i == 107 || i == 112 {
            continue;
        }
        // Spec-mandated formula: rotate right by 1, then add the byte.
        csum = csum.rotate_right(1).wrapping_add(b as u32);
    }
    csum
}

// -- Upcase table -------------------------------------------------------

/// Generate a 131 072-byte (65 536 × u16 LE) upcase table.
///
/// This covers:
///   - ASCII          a–z  → A–Z
///   - Latin-1 supp.  à–ö  → À–Ö,  ø–þ  → Ø–Þ
///   - Cyrillic       а–я  → А–Я
///   - All other code points: identity mapping
///
/// The resulting CRC-32 is computed and stored in the `UpcaseTableEntry`
/// in the root directory.
///
/// NOTE: The Microsoft reference table (CRC `0xE619_D30D`) maps the full
/// Unicode BMP.  This simplified table is sufficient for ASCII and common
/// European filenames; volumes formatted here may differ from Windows in
/// upper-range case-insensitive comparisons.
fn generate_upcase_table() -> Vec<u8> {
    let mut out = Vec::with_capacity(consts::UPCASE_TABLE_CHARS * 2);
    for cp in 0u32..=0xFFFF {
        let up: u16 = match cp {
            0x0061..=0x007A => (cp - 0x20) as u16, // a–z      → A–Z
            0x00E0..=0x00F6 => (cp - 0x20) as u16, // à–ö      → À–Ö
            0x00F8..=0x00FE => (cp - 0x20) as u16, // ø–þ      → Ø–Þ
            0x0430..=0x044F => (cp - 0x20) as u16, // Cyrillic а–я → А–Я
            _ => cp as u16,
        };
        out.extend_from_slice(&up.to_le_bytes());
    }
    out
}

/// Standard CRC-32 (Ethernet / ISO 3309, big-endian shift variant) as
/// required by the ExFAT spec for the upcase-table checksum field.
fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in data {
        crc ^= (b as u32) << 24;
        for _ in 0..8 {
            crc = if crc & 0x8000_0000 != 0 {
                (crc << 1) ^ 0x04C1_1DB7
            }
            else {
                crc << 1
            };
        }
    }
    !crc
}

impl MakeFileSystem for ExfatFileSystem {
    fn mkfs(stream: &mut File, size: u64) -> io::Result<()> {
        if size < consts::MINIMUM_IMAGE_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "file size too small for ExFAT",
            ));
        }

        // Choose sectors per cluster based on image size.
        let spc = if size < 256 * 1024 * 1024 {
            // < 256 MiB
            4
        }
        else if size < 1 * 1024 * 1024 * 1024 {
            // < 1 GiB
            8
        }
        else if size < 128_u64 * 1024 * 1024 * 1024 {
            // < 128 GiB
            16
        }
        else if size < 512_u64 * 1024 * 1024 * 1024 {
            // < 512 GiB
            32
        }
        else if size <= 2_u64 * 1024 * 1024 * 1024 * 1024 {
            // <= 2 TiB
            64
        }
        else {
            128
        };

        let bps = 1_u64 << consts::MAIN_SECTOR_SHIFT; // 512 bytes/sector
        let cluster_size: u64 = spc * bps;

        // Extend the file to exactly `size` bytes; the OS zero-fills the extension.
        // stream.set_len(size)?;

        // -- Layout arithmetic -----------------------------------------------------
        //
        // The boot region occupies the first 24 sectors (12 main + 12 backup).
        // The FAT starts immediately after, aligned to a cluster boundary so that
        // the cluster heap that follows it is also cluster-aligned.

        let volume_length: u64 = size / bps;
        let fat_offset: u64 = align_up(24, spc);

        // -- Iterative fat_length / cluster_count resolution -----------------------
        //
        // The problem is circular:
        //   fat_length  depends on cluster_count  (1 FAT entry per cluster)
        //   cluster_count depends on fat_length   (FAT consumes sectors)
        //
        // Solution: over-estimate cluster_count first (pretend FAT = 0 bytes),
        // derive fat_length, then compute the real cluster_count from what remains.
        // One sanity-check pass is sufficient because the estimate can only shrink.

        // Pass 1 — estimate.
        let cc_est: u64 = (volume_length - fat_offset) / spc;
        let fl_raw: u64 = ((cc_est + 2) * 4 + bps - 1) / bps; // ceil((cc+2)*4 / bps)
        let fat_length: u64 = align_up(fl_raw, spc); // align to cluster

        // Pass 2 — real cluster_count with actual FAT size.
        let cluster_heap_offset: u64 = fat_offset + fat_length;
        let cluster_count: u64 = (volume_length - cluster_heap_offset) / spc;

        // Pass 3 — verify fat_length is still large enough (edge case: extremely
        // small volumes where the FAT rounding consumed a measurable fraction).
        let fat_length: u64 = {
            let needed = ((cluster_count + 2) * 4 + bps - 1) / bps;
            if needed > fat_length {
                align_up(needed, spc) // one extra cluster-group covers it
            }
            else {
                fat_length
            }
        };

        // Recompute with final fat_length.
        let cluster_heap_offset: u64 = fat_offset + fat_length;
        let cluster_count: u64 = (volume_length - cluster_heap_offset) / spc;

        if cluster_count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "volume too small: no room for data clusters after FAT",
            ));
        }

        // -- Cluster assignments --------------------------------------------------
        //
        // Cluster numbering starts at 2 (clusters 0 and 1 are reserved in the FAT
        // but have no corresponding physical storage in the cluster heap).
        //
        //   [2 … 2+B-1]          Allocation bitmap
        //   [2+B … 2+B+U-1]      Upcase table
        //   [2+B+U]              Root directory  (one cluster, empty)

        let bitmap_start: u64 = consts::FIRST_DATA_CLUSTER as u64; // always 2
        let bitmap_bytes: u64 = (cluster_count + 7) / 8; // 1 bit per cluster
        let bitmap_clusters: u64 = (bitmap_bytes + cluster_size - 1) / cluster_size;

        let upcase_data: Vec<u8> = generate_upcase_table();
        let upcase_crc: u32 = crc32(&upcase_data);
        let upcase_bytes: u64 = upcase_data.len() as u64; // 131 072
        let upcase_start: u64 = bitmap_start + bitmap_clusters;
        let upcase_clusters: u64 = (upcase_bytes + cluster_size - 1) / cluster_size;

        let root_start: u64 = upcase_start + upcase_clusters;
        let used_clusters: u64 = bitmap_clusters + upcase_clusters + 1; // +1 for root

        if root_start >= cluster_count + 2 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "volume too small: metadata clusters exceed cluster heap",
            ));
        }

        let percent_in_use: u8 =
            ((used_clusters.saturating_mul(100) / cluster_count) as u8).min(100);

        // -- Build Main Boot Sector ---------------------------------------------

        // Volume serial number: mix timestamp seconds and nanoseconds for uniqueness.
        let serial: u32 = {
            let now = Local::now();
            let a = now.timestamp() as u64;
            let b = (u32::MAX as u64).wrapping_mul(a);
            b.wrapping_add(now.timestamp_subsec_nanos() as u64) as u32
        };

        let mut mbs = MainBootSector::default();
        mbs.volume_length = volume_length;
        mbs.fat_offset = fat_offset as u32;
        mbs.fat_length = fat_length as u32;
        mbs.cluster_heap_offset = cluster_heap_offset as u32;
        mbs.cluster_count = cluster_count as u32;
        mbs.root_directory_cluster = root_start as u32;
        mbs.volume_serial_number = serial;
        mbs.bytes_per_sector_shift = consts::MAIN_SECTOR_SHIFT as u8; // log2(512) = 9
        mbs.sectors_per_cluster_shift = spc.trailing_zeros() as u8;
        mbs.percent_in_use = percent_in_use;

        let mbs_bytes: Vec<u8> = mbs.to_bytes().to_vec(); // exactly 512 bytes
        debug_assert_eq!(mbs_bytes.len(), 512);

        // -- Build the remaining boot-region sectors -----------------------------

        // Sectors 1–8: Extended boot sectors.
        // Each is 512 zero bytes with EXTENDED_BOOT_SIGNATURE at bytes 508–511.
        let mut ext_sector = vec![0u8; 512];
        ext_sector[508..512].copy_from_slice(&consts::EXTENDED_BOOT_SIGNATURE.to_le_bytes());

        // Sectors 9 (OEM parameters) and 10 (reserved): all zeros.
        let zeroed_sector = vec![0u8; 512];

        // -- Assemble sectors 0–10 for checksum computation -----------------
        //
        // `boot_checksum` skips bytes 106, 107 (VolumeFlags) and 112 (PercentInUse)
        // so those fields can change during normal operation without invalidating it.
        let mut sectors_0_to_10: Vec<u8> = Vec::with_capacity(512 * 11);
        sectors_0_to_10.extend_from_slice(&mbs_bytes); // sector 0
        for _ in 0..8 {
            sectors_0_to_10.extend_from_slice(&ext_sector); // sectors 1–8
        }
        sectors_0_to_10.extend_from_slice(&zeroed_sector); // sector 9
        sectors_0_to_10.extend_from_slice(&zeroed_sector); // sector 10
        debug_assert_eq!(sectors_0_to_10.len(), 512 * 11);

        let csum = boot_checksum(&sectors_0_to_10);

        // Sector 11: the u32 checksum value repeated 128 times (fills all 512 bytes).
        let mut checksum_sector = vec![0u8; 512];
        for i in (0..512).step_by(4) {
            checksum_sector[i..i + 4].copy_from_slice(&csum.to_le_bytes());
        }

        // -- Write main boot region (sectors 0–11) ------------------------
        stream.seek(SeekFrom::Start(0))?;
        stream.write_all(&sectors_0_to_10)?;
        stream.write_all(&checksum_sector)?;

        // -- Write backup boot region (sectors 12–23) ---------------------
        // Spec requires an exact copy; a repair tool can restore the main region
        // from this backup if the main region is found to be corrupt.
        stream.write_all(&sectors_0_to_10)?;
        stream.write_all(&checksum_sector)?;

        // -- Write the FAT -----------------------------------------------
        //
        // The FAT is a flat array of u32 entries, one per cluster.
        // Index 0 and 1 are reserved; real data clusters start at index 2.
        //
        // Chains are written by storing the *next* cluster number in each entry
        // and EXFAT_EOC (0xFFFFFFFF) in the last entry of each chain.
        let fat_entry_count = (fat_length * bps / 4) as usize;
        let mut fat: Vec<u32> = vec![consts::EXFAT_FREE; fat_entry_count];

        fat[0] = consts::EXFAT_MEDIA; // media descriptor (required by spec)
        fat[1] = consts::EXFAT_EOC; // end-of-chain sentinel for reserved slot

        // Allocation bitmap chain.
        for i in 0..bitmap_clusters as usize {
            let c = bitmap_start as usize + i;
            fat[c] = if i + 1 < bitmap_clusters as usize {
                c as u32 + 1
            }
            else {
                consts::EXFAT_EOC
            };
        }

        // Upcase table chain.
        for i in 0..upcase_clusters as usize {
            let c = upcase_start as usize + i;
            fat[c] = if i + 1 < upcase_clusters as usize {
                c as u32 + 1
            }
            else {
                consts::EXFAT_EOC
            };
        }

        // Root directory occupies exactly one cluster.
        fat[root_start as usize] = consts::EXFAT_EOC;
        
        // Print the Main Boot Sector.
        println!("{:?}", mbs);
        let fat_raw: Vec<u8> = fat.iter().flat_map(|x| x.to_le_bytes()).collect();
        stream.seek(SeekFrom::Start(fat_offset * bps))?;
        stream.write_all(&fat_raw)?;

        // -- Write allocation bitmap ----------------------------------------------
        //
        // The bitmap has one bit per cluster.  Bit index k represents cluster k+2.
        // Bit = 0 -> free,  bit = 1 -> allocated.
        //
        // We pre-mark the clusters consumed by the bitmap itself, the upcase table,
        // and the root directory as allocated.  Everything else starts as free (0).
        let mut bitmap_buf = vec![0u8; (bitmap_clusters * cluster_size) as usize];
        for k in 0..used_clusters as usize {
            bitmap_buf[k / 8] |= 1 << (k % 8);
        }
        let bmp_off = mbs.cluster_byte_offset(bitmap_start as u32);
        stream.seek(SeekFrom::Start(bmp_off))?;
        stream.write_all(&bitmap_buf)?;

        // -- Step 10: Write upcase table -------------------------------------------
        //
        // Pad to a whole number of clusters with zeros; the DataLength field in the
        // directory entry records the exact byte count so readers stop at the right
        // place.
        let mut upcase_buf = vec![0u8; (upcase_clusters * cluster_size) as usize];
        upcase_buf[..upcase_data.len()].copy_from_slice(&upcase_data);
        let uc_off = mbs.cluster_byte_offset(upcase_start as u32);
        stream.seek(SeekFrom::Start(uc_off))?;
        stream.write_all(&upcase_buf)?;

        // -- Write root directory ---------------------------------------
        //
        // A freshly formatted ExFAT root contains exactly two mandatory entries:
        //   1. Allocation Bitmap (type 0x81)
        //   2. Upcase Table      (type 0x82)
        //
        // Both are 32 bytes each (the standard ExFAT entry size).
        // The rest of the cluster is zeroed; a 0x00 first-byte terminates scanning.
        let mut root_buf = vec![0u8; cluster_size as usize];

        // -- Entry 0: Allocation Bitmap  -----------------------------------------
        //
        // Offset   Length   Field
        //      0        1   EntryType      = 0x81
        //      1        1   BitmapFlags    = 0x00 (first / only bitmap)
        //   2–19       18   Reserved       (zeros)
        //  20–23        4   FirstCluster   (LE u32)
        //  24–31        8   DataLength     (LE u64, in bytes)
        {
            let e = &mut root_buf[0..32];
            e[0] = consts::TYPE_ALLOC_BITMAP;
            e[1] = 0x00; // BitmapFlags: corresponds to FAT 0
            e[20..24].copy_from_slice(&(bitmap_start as u32).to_le_bytes());
            e[24..32].copy_from_slice(&bitmap_bytes.to_le_bytes());
        }

        // -- Entry 1: Upcase Table -----------------------------------------------
        //
        // Offset   Length   Field
        //     0         1   EntryType      = 0x82
        //   1–3         3   Reserved1      (zeros)
        //   4–7         4   TableChecksum  (CRC-32 of the table data, LE u32)
        //  8–19        12   Reserved2      (zeros)
        //  20–23        4   FirstCluster   (LE u32)
        //  24–31        8   DataLength     (LE u64, in bytes)
        {
            let e = &mut root_buf[32..64];
            e[0] = consts::TYPE_UPCASE_TABLE;
            e[4..8].copy_from_slice(&upcase_crc.to_le_bytes());
            e[20..24].copy_from_slice(&(upcase_start as u32).to_le_bytes());
            e[24..32].copy_from_slice(&upcase_bytes.to_le_bytes());
        }

        let root_off = mbs.cluster_byte_offset(root_start as u32);
        stream.seek(SeekFrom::Start(root_off))?;
        stream.write_all(&root_buf)?;

        Ok(())
    }
}
