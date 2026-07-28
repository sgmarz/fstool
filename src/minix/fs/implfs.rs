//! Minix 3 File System Implementation
//!
//! © Stephen Marz
//! 8 June 2026
use crate::cache::Tree;
use crate::fs::{FileSystem, FileType, InodeOperations, SuperblockOperations};
use crate::minix::{DirEntry, MinixFileSystem, consts, get_indirect_zone};
use crate::minix::{
    consts::{DIR_ENTRY_BYTES, INDIRECT_ZONE},
    fs::create,
};
use crate::path::{fname_from_path, remove_item, split_items};
use std::io::{self, Error, ErrorKind, Read, Seek, SeekFrom, Write};

impl FileSystem for MinixFileSystem {
    /// Get the name of the file system.
    fn name(&self) -> &str {
        "minix"
    }

    /// Read a single block into the `data` buffer.
    ///
    /// The block number is relative to the very start of the file system. For example,
    /// block 0 is the first block (the boot block), and block 1 would be the superblock.
    ///
    /// The buffer must be at least as large as the block size of the file system, but it
    /// may be larger. If the buffer is larger, the implementation should only read up to the block size
    /// and leave the rest of the buffer unchanged.
    fn read_block(&mut self, block_num: u64, data: &mut [u8]) -> io::Result<()> {
        if block_num < self.superblock.first_data_zone as u64 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "block {} is out of bounds. First data zone is {}",
                    block_num, self.superblock.first_data_zone
                ),
            ));
        }
        else if block_num >= self.superblock.num_zones as u64 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "block number {} is out of bounds. Max block number is {}",
                    block_num,
                    self.superblock.num_zones - 1
                ),
            ));
        }
        let size = usize::min(self.superblock.block_size as usize, data.len());
        if self.bcache.copy_block_to(block_num, &mut data[..size]) {
            return Ok(());
        }
        let location = block_num * self.superblock.block_size as u64;
        self.stream.seek(SeekFrom::Start(location))?;
        let mut block_data = vec![0u8; self.superblock.block_size as usize];
        self.stream.read_exact(&mut block_data[..size])?;
        data[..size].copy_from_slice(&block_data[..size]);
        self.bcache.insert_read(block_num, block_data);
        Ok(())
    }

    /// Write data to a block. The data may be bigger than a block, but only the block
    /// will be written.
    ///
    /// This first writes to cache.
    fn write_block(&mut self, block_num: u64, data: &[u8]) -> io::Result<()> {
        let block_size = usize::min(self.superblock.block_size as usize, data.len());
        if !self
            .bcache
            .insert_write(block_num, data[..block_size].to_vec())
        {
            return Err(io::Error::new(
                io::ErrorKind::OutOfMemory,
                "block cache insert failed",
            ));
        }
        Ok(())
    }

    fn read_file(&mut self, inode_num: u64, offset: u64, buffer: &mut [u8]) -> io::Result<u64> {
        let buffer_len = buffer.len() as u64;
        let block_size = self.superblock.block_size as u64;
        let inode = self.get_inode(inode_num)?;
        let file_size = inode.get_size();
        if offset >= file_size {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "offset is beyond end of file",
            ));
        }
        let mut offset = offset;
        let bytes_to_read = u64::min(buffer_len, file_size - offset);
        let mut bytes_read = 0u64;
        let file_blocks = inode.get_blocks();
        let mut byte_at = 0;
        let mut block_data = vec![0u8; block_size as usize];
        for &b in file_blocks[..consts::INDIRECT_ZONE as usize]
            .iter()
            .filter(|&&b| b != 0)
        {
            if bytes_read >= bytes_to_read {
                return Ok(bytes_read as u64);
            }
            byte_at += block_size;
            if byte_at <= offset {
                continue;
            }
            offset = offset % block_size;
            self.read_block(b, &mut block_data)?;
            let bytes_from_block = u64::min(block_size - offset, bytes_to_read - bytes_read);
            buffer[bytes_read as usize..((bytes_read + bytes_from_block) as usize)]
                .copy_from_slice(
                    &block_data[offset as usize..(offset as usize + bytes_from_block as usize)],
                );
            bytes_read += bytes_from_block;
            offset = 0;
        }
        let indirect_block = file_blocks[consts::INDIRECT_ZONE as usize];
        if indirect_block != 0 {
            let pointers = get_indirect_zone(self, indirect_block)?
                .into_iter()
                .filter(|&b| b != 0);
            for b in pointers {
                if bytes_read >= bytes_to_read {
                    return Ok(bytes_read as u64);
                }
                byte_at += block_size;
                if byte_at <= offset {
                    continue;
                }
                offset = offset % block_size;
                self.read_block(b as u64, &mut block_data)?;
                let bytes_from_block = u64::min(block_size - offset, bytes_to_read - bytes_read);
                buffer[(bytes_read as usize)..((bytes_read + bytes_from_block) as usize)]
                    .copy_from_slice(
                        &block_data[offset as usize..(offset as usize + bytes_from_block as usize)],
                    );
                bytes_read += bytes_from_block;
                offset = 0;
            }
        }
        let dind_block = file_blocks[consts::DINDIRECT_ZONE as usize];
        if dind_block != 0 {
            let indirect_pointers = get_indirect_zone(self, dind_block)?
                .into_iter()
                .filter(|&b| b != 0);
            for indirect_block in indirect_pointers {
                let pointers = get_indirect_zone(self, indirect_block as u64)?
                    .into_iter()
                    .filter(|&b| b != 0);
                for b in pointers {
                    if bytes_read >= bytes_to_read {
                        return Ok(bytes_read as u64);
                    }
                    byte_at += block_size;
                    if byte_at <= offset {
                        continue;
                    }
                    offset = offset % block_size;
                    self.read_block(b as u64, &mut block_data)?;
                    let bytes_from_block =
                        u64::min(block_size - offset, bytes_to_read - bytes_read);
                    buffer[(bytes_read as usize)..((bytes_read + bytes_from_block) as usize)]
                        .copy_from_slice(
                            &block_data
                                [offset as usize..(offset as usize + bytes_from_block as usize)],
                        );
                    bytes_read += bytes_from_block;
                    offset = 0;
                }
            }
        }
        let tind_block = file_blocks[consts::TINDIRECT_ZONE as usize];
        if tind_block != 0 {
            let dind_pointers = get_indirect_zone(self, tind_block)?
                .into_iter()
                .filter(|&b| b != 0);
            for dind_block in dind_pointers {
                let indirect_pointers = get_indirect_zone(self, dind_block as u64)?
                    .into_iter()
                    .filter(|&b| b != 0);
                for indirect_block in indirect_pointers {
                    let pointers = get_indirect_zone(self, indirect_block as u64)?
                        .into_iter()
                        .filter(|&b| b != 0);
                    for b in pointers {
                        if bytes_read >= bytes_to_read {
                            return Ok(bytes_read as u64);
                        }
                        byte_at += block_size;
                        if byte_at <= offset {
                            continue;
                        }
                        offset = offset % block_size;
                        self.read_block(b as u64, &mut block_data)?;
                        let bytes_from_block =
                            u64::min(block_size - offset, bytes_to_read - bytes_read);
                        buffer[(bytes_read as usize)..((bytes_read + bytes_from_block) as usize)]
                            .copy_from_slice(
                                &block_data[offset as usize
                                    ..(offset as usize + bytes_from_block as usize)],
                            );
                        bytes_read += bytes_from_block;
                        offset = 0;
                    }
                }
            }
        }
        Ok(bytes_read as u64)
    }

    fn write_file(&mut self, inode_num: u64, offset: u64, buffer: &[u8]) -> io::Result<u64> {
        let bsize = self.superblock.block_size as u64;
        let buffer_len = buffer.len() as u64;

        if buffer_len == 0 {
            return Ok(0);
        }

        // Grow the file if this write extends past the current end.
        // The scope ensures the inode borrow is released before truncate borrows self.
        {
            let current_size = self.get_inode(inode_num)?.get_size();
            let end_pos = offset + buffer_len;
            if end_pos > current_size {
                self.truncate(inode_num, end_pos)?;
            }
        }

        // Snapshot the block map now that truncate may have allocated new blocks.
        let file_blocks = self.get_inode(inode_num)?.get_blocks();

        let mut bytes_written = 0u64;
        let mut cur_offset = offset; // Tracks remaining bytes-to-skip into the file.
        let mut byte_at = 0u64; // Cumulative byte position after each block.
        let mut block_data = vec![0u8; bsize as usize];

        // Helper macro to avoid repeating the read-patch-write pattern.
        macro_rules! write_to_block {
            ($blk:expr) => {{
                if bytes_written >= buffer_len {
                    return Ok(bytes_written);
                }
                byte_at += bsize;
                if byte_at <= cur_offset {
                    // This block lies entirely before our write offset.
                }
                else {
                    let blk_off = cur_offset % bsize;
                    self.read_block($blk as u64, &mut block_data)?;
                    let to_write = u64::min(bsize - blk_off, buffer_len - bytes_written);
                    block_data[blk_off as usize..(blk_off + to_write) as usize].copy_from_slice(
                        &buffer[bytes_written as usize..(bytes_written + to_write) as usize],
                    );
                    self.write_block($blk as u64, &block_data)?;
                    bytes_written += to_write;
                    cur_offset = 0;
                }
            }};
        }

        // Direct blocks
        for &b in file_blocks[..consts::INDIRECT_ZONE as usize]
            .iter()
            .filter(|&&b| b != 0)
        {
            write_to_block!(b);
        }

        // Single indirect
        let ind_blk = file_blocks[consts::INDIRECT_ZONE as usize];
        if ind_blk != 0 && bytes_written < buffer_len {
            let ptrs = get_indirect_zone(self, ind_blk)?;
            for b in ptrs.into_iter().filter(|&b| b != 0) {
                write_to_block!(b);
            }
        }

        // Double indirect
        let dind_blk = file_blocks[consts::DINDIRECT_ZONE as usize];
        if dind_blk != 0 && bytes_written < buffer_len {
            let ind_ptrs = get_indirect_zone(self, dind_blk)?;
            for ind_b in ind_ptrs.into_iter().filter(|&b| b != 0) {
                if bytes_written >= buffer_len {
                    break;
                }
                let ptrs = get_indirect_zone(self, ind_b as u64)?;
                for b in ptrs.into_iter().filter(|&b| b != 0) {
                    write_to_block!(b);
                }
            }
        }

        // Triple indirect
        let tind_blk = file_blocks[consts::TINDIRECT_ZONE as usize];
        if tind_blk != 0 && bytes_written < buffer_len {
            let dind_ptrs = get_indirect_zone(self, tind_blk)?;
            for dind_b in dind_ptrs.into_iter().filter(|&b| b != 0) {
                if bytes_written >= buffer_len {
                    break;
                }
                let ind_ptrs = get_indirect_zone(self, dind_b as u64)?;
                for ind_b in ind_ptrs.into_iter().filter(|&b| b != 0) {
                    if bytes_written >= buffer_len {
                        break;
                    }
                    let ptrs = get_indirect_zone(self, ind_b as u64)?;
                    for b in ptrs.into_iter().filter(|&b| b != 0) {
                        write_to_block!(b);
                    }
                }
            }
        }

        Ok(bytes_written)
    }

    /// ## Set the size of a file by inode.
    ///
    /// If the size > old size, allocate the blocks as needed.
    /// If the size == old size, nothing is done.
    /// If the size < old size, deallocate the blocks as needed.
    ///
    /// This function is called when unlink sets the links to 0.
    fn truncate(&mut self, inode_num: u64, size: u64) -> io::Result<u64> {
        let bsize = self.superblock.block_size as u64;
        assert!(bsize > 0 && bsize.is_power_of_two());

        {
            // Make sure this is something that actually has a size.
            let inode = self.get_inode(inode_num)?;
            match inode.get_file_type() {
                FileType::Directory | FileType::Regular | FileType::Symlink => {}
                // Everything else is a device, which doesn't have blocks to allocate or free.
                _ => {
                    return Ok(size);
                }
            }
        }

        let old_size = { self.get_inode(inode_num)?.get_size() };
        if size == old_size {
            return Ok(size);
        }

        // Each zone pointer is a u32 (4 bytes).
        let ptrs_per_block = (bsize / 4) as usize;
        let n_direct = consts::INDIRECT_ZONE as usize;

        let old_num_blocks = ((old_size + bsize - 1) / bsize) as usize;
        let new_num_blocks = ((size + bsize - 1) / bsize) as usize;

        // Pre-computed zone-range boundaries (in linear block-index space).
        let ind_zone_start = n_direct;
        let ind_zone_end = ind_zone_start + ptrs_per_block;
        let dind_zone_start = ind_zone_end;
        let dind_zone_end = dind_zone_start + ptrs_per_block * ptrs_per_block;
        let tind_zone_start = dind_zone_end;

        // ------------------------------------------------------------------
        // Helpers: read a u32 little-endian pointer from a raw block buffer.
        // ------------------------------------------------------------------
        fn read_ptr(buf: &[u8], idx: usize) -> u64 {
            let off = idx * 4;
            u32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]]) as u64
        }
        fn write_ptr(buf: &mut [u8], idx: usize, val: u64) {
            let off = idx * 4;
            let bytes = (val as u32).to_le_bytes();
            buf[off..off + 4].copy_from_slice(&bytes);
        }
        // Determine if we need to grow, shrink, or just leave it alone.
        if size > old_size {
            // ==============================================================
            // GROW: allocate blocks old_num_blocks..new_num_blocks
            // ==============================================================
            let zeros = vec![0u8; bsize as usize];

            for block_idx in old_num_blocks..new_num_blocks {
                if block_idx < n_direct {
                    // ---- Direct ----
                    let nb = self.get_superblock_mut().allocate_block()?;
                    self.write_block(nb, &zeros)?;
                    let mut blocks = self.get_inode(inode_num)?.get_blocks();
                    blocks[block_idx] = nb;
                    self.get_inode_mut(inode_num)?.set_blocks(&blocks);
                }
                else if block_idx < ind_zone_end {
                    // ---- Single indirect ----
                    let ind_idx = block_idx - ind_zone_start;

                    // Allocate the pointer block if this is the very first
                    // indirect entry we're touching.
                    let ind_ptr_blk =
                        { self.get_inode(inode_num)?.get_blocks()[consts::INDIRECT_ZONE as usize] };
                    let ind_ptr_blk = if ind_ptr_blk == 0 {
                        let nb = self.get_superblock_mut().allocate_block()?;
                        self.write_block(nb, &zeros)?;
                        let mut blocks = self.get_inode(inode_num)?.get_blocks();
                        blocks[consts::INDIRECT_ZONE as usize] = nb;
                        self.get_inode_mut(inode_num)?.set_blocks(&blocks);
                        nb
                    }
                    else {
                        ind_ptr_blk
                    };

                    let nb = self.get_superblock_mut().allocate_block()?;
                    self.write_block(nb, &zeros)?;
                    let mut buf = zeros.clone();
                    self.read_block(ind_ptr_blk, &mut buf)?;
                    write_ptr(&mut buf, ind_idx, nb);
                    self.write_block(ind_ptr_blk, &buf)?;
                }
                else if block_idx < dind_zone_end {
                    // ---- Double indirect ----
                    let drel = block_idx - dind_zone_start;
                    let ind_i = drel / ptrs_per_block;
                    let blk_i = drel % ptrs_per_block;

                    // Level 1: dind pointer block
                    let dind_ptr_blk = {
                        self.get_inode(inode_num)?.get_blocks()[consts::DINDIRECT_ZONE as usize]
                    };
                    let dind_ptr_blk = if dind_ptr_blk == 0 {
                        let nb = self.get_superblock_mut().allocate_block()?;
                        self.write_block(nb, &zeros)?;
                        let mut blocks = self.get_inode(inode_num)?.get_blocks();
                        blocks[consts::DINDIRECT_ZONE as usize] = nb;
                        self.get_inode_mut(inode_num)?.set_blocks(&blocks);
                        nb
                    }
                    else {
                        dind_ptr_blk
                    };

                    // Level 2: indirect pointer block within dind
                    let mut dind_buf = zeros.clone();
                    self.read_block(dind_ptr_blk, &mut dind_buf)?;
                    let ind_ptr_blk = read_ptr(&dind_buf, ind_i);
                    let ind_ptr_blk = if ind_ptr_blk == 0 {
                        let nb = self.get_superblock_mut().allocate_block()?;
                        self.write_block(nb, &zeros)?;
                        write_ptr(&mut dind_buf, ind_i, nb);
                        self.write_block(dind_ptr_blk, &dind_buf)?;
                        nb
                    }
                    else {
                        ind_ptr_blk
                    };

                    // Data block
                    let nb = self.get_superblock_mut().allocate_block()?;
                    self.write_block(nb, &zeros)?;
                    let mut ind_buf = zeros.clone();
                    self.read_block(ind_ptr_blk, &mut ind_buf)?;
                    write_ptr(&mut ind_buf, blk_i, nb);
                    self.write_block(ind_ptr_blk, &ind_buf)?;
                }
                else {
                    // ---- Triple indirect ----
                    let trel = block_idx - tind_zone_start;
                    let ppb2 = ptrs_per_block * ptrs_per_block;
                    let dind_i = trel / ppb2;
                    let ind_i = (trel % ppb2) / ptrs_per_block;
                    let blk_i = trel % ptrs_per_block;

                    // Level 1: tind pointer block
                    let tind_ptr_blk = {
                        self.get_inode(inode_num)?.get_blocks()[consts::TINDIRECT_ZONE as usize]
                    };
                    let tind_ptr_blk = if tind_ptr_blk == 0 {
                        let nb = self.get_superblock_mut().allocate_block()?;
                        self.write_block(nb, &zeros)?;
                        let mut blocks = self.get_inode(inode_num)?.get_blocks();
                        blocks[consts::TINDIRECT_ZONE as usize] = nb;
                        self.get_inode_mut(inode_num)?.set_blocks(&blocks);
                        nb
                    }
                    else {
                        tind_ptr_blk
                    };

                    // Level 2: dind pointer block within tind
                    let mut tind_buf = zeros.clone();
                    self.read_block(tind_ptr_blk, &mut tind_buf)?;
                    let dind_ptr_blk = read_ptr(&tind_buf, dind_i);
                    let dind_ptr_blk = if dind_ptr_blk == 0 {
                        let nb = self.get_superblock_mut().allocate_block()?;
                        self.write_block(nb, &zeros)?;
                        write_ptr(&mut tind_buf, dind_i, nb);
                        self.write_block(tind_ptr_blk, &tind_buf)?;
                        nb
                    }
                    else {
                        dind_ptr_blk
                    };

                    // Level 3: indirect pointer block within dind
                    let mut dind_buf = zeros.clone();
                    self.read_block(dind_ptr_blk, &mut dind_buf)?;
                    let ind_ptr_blk = read_ptr(&dind_buf, ind_i);
                    let ind_ptr_blk = if ind_ptr_blk == 0 {
                        let nb = self.get_superblock_mut().allocate_block()?;
                        self.write_block(nb, &zeros)?;
                        write_ptr(&mut dind_buf, ind_i, nb);
                        self.write_block(dind_ptr_blk, &dind_buf)?;
                        nb
                    }
                    else {
                        ind_ptr_blk
                    };

                    // Data block
                    let nb = self.get_superblock_mut().allocate_block()?;
                    self.write_block(nb, &zeros)?;
                    let mut ind_buf = zeros.clone();
                    self.read_block(ind_ptr_blk, &mut ind_buf)?;
                    write_ptr(&mut ind_buf, blk_i, nb);
                    self.write_block(ind_ptr_blk, &ind_buf)?;
                }
            }
        }
        else {
            // ==============================================================
            // SHRINK: free blocks new_num_blocks..old_num_blocks
            // ==============================================================
            let bsz = bsize as usize;

            // ---- 1. Direct blocks ----
            let dir_from = new_num_blocks.min(n_direct);
            let dir_to = old_num_blocks.min(n_direct);
            for i in dir_from..dir_to {
                let blk = { self.get_inode(inode_num)?.get_blocks()[i] };
                if blk != 0 {
                    let _ = self.get_superblock_mut().deallocate_block(blk);
                    let mut blocks = self.get_inode(inode_num)?.get_blocks();
                    blocks[i] = 0;
                    self.get_inode_mut(inode_num)?.set_blocks(&blocks);
                }
            }

            // ---- 2. Single indirect ----
            if old_num_blocks > ind_zone_start {
                let ind_ptr_blk =
                    { self.get_inode(inode_num)?.get_blocks()[consts::INDIRECT_ZONE as usize] };
                if ind_ptr_blk != 0 {
                    // Range of entries within the indirect block to free.
                    let entry_from = new_num_blocks
                        .saturating_sub(ind_zone_start)
                        .min(ptrs_per_block);
                    let entry_to = (old_num_blocks - ind_zone_start).min(ptrs_per_block);

                    let mut buf = vec![0u8; bsz];
                    self.read_block(ind_ptr_blk, &mut buf)?;
                    for i in entry_from..entry_to {
                        let blk = read_ptr(&buf, i);
                        if blk != 0 {
                            let _ = self.get_superblock_mut().deallocate_block(blk);
                            write_ptr(&mut buf, i, 0);
                        }
                    }
                    self.write_block(ind_ptr_blk, &buf)?;

                    // Free the pointer block itself if no indirect entries remain.
                    if new_num_blocks <= ind_zone_start {
                        let _ = self.get_superblock_mut().deallocate_block(ind_ptr_blk);
                        let mut blocks = self.get_inode(inode_num)?.get_blocks();
                        blocks[consts::INDIRECT_ZONE as usize] = 0;
                        self.get_inode_mut(inode_num)?.set_blocks(&blocks);
                    }
                }
            }

            // ---- 3. Double indirect ----
            if old_num_blocks > dind_zone_start {
                let dind_ptr_blk =
                    { self.get_inode(inode_num)?.get_blocks()[consts::DINDIRECT_ZONE as usize] };
                if dind_ptr_blk != 0 {
                    // Relative range within the dind address space.
                    let drel_from = new_num_blocks
                        .saturating_sub(dind_zone_start)
                        .min(ptrs_per_block * ptrs_per_block);
                    let drel_to =
                        (old_num_blocks - dind_zone_start).min(ptrs_per_block * ptrs_per_block);

                    // Which indirect pointer blocks are affected?
                    let ind_from = drel_from / ptrs_per_block;
                    let ind_to = (drel_to + ptrs_per_block - 1) / ptrs_per_block;

                    let mut dind_buf = vec![0u8; bsz];
                    self.read_block(dind_ptr_blk, &mut dind_buf)?;
                    let mut dind_dirty = false;

                    for ind_i in ind_from..ind_to {
                        let ind_ptr_blk = read_ptr(&dind_buf, ind_i);
                        if ind_ptr_blk != 0 {
                            // Which data-block entries inside this indirect block to free?
                            let blk_start = if ind_i == ind_from {
                                drel_from % ptrs_per_block
                            }
                            else {
                                0
                            };
                            let blk_end = {
                                let abs_end = drel_to.min((ind_i + 1) * ptrs_per_block);
                                abs_end - ind_i * ptrs_per_block
                            };

                            let mut ind_buf = vec![0u8; bsz];
                            self.read_block(ind_ptr_blk, &mut ind_buf)?;
                            for j in blk_start..blk_end {
                                let blk = read_ptr(&ind_buf, j);
                                if blk != 0 {
                                    let _ = self.get_superblock_mut().deallocate_block(blk);
                                    write_ptr(&mut ind_buf, j, 0);
                                }
                            }
                            self.write_block(ind_ptr_blk, &ind_buf)?;

                            // Free this indirect pointer block if we cleared it from the
                            // start (meaning no kept entries precede the freed range).
                            if blk_start == 0 {
                                let _ = self.get_superblock_mut().deallocate_block(ind_ptr_blk);
                                write_ptr(&mut dind_buf, ind_i, 0);
                                dind_dirty = true;
                            }
                        }
                    }

                    if dind_dirty {
                        self.write_block(dind_ptr_blk, &dind_buf)?;
                    }

                    // Free the dind pointer block if no indirect entries remain.
                    if new_num_blocks <= dind_zone_start {
                        let _ = self.get_superblock_mut().deallocate_block(dind_ptr_blk);
                        let mut blocks = self.get_inode(inode_num)?.get_blocks();
                        blocks[consts::DINDIRECT_ZONE as usize] = 0;
                        self.get_inode_mut(inode_num)?.set_blocks(&blocks);
                    }
                }
            }

            // ---- 4. Triple indirect ----
            if old_num_blocks > tind_zone_start {
                let tind_ptr_blk =
                    { self.get_inode(inode_num)?.get_blocks()[consts::TINDIRECT_ZONE as usize] };
                if tind_ptr_blk != 0 {
                    let ppb2 = ptrs_per_block * ptrs_per_block;

                    let trel_from = new_num_blocks
                        .saturating_sub(tind_zone_start)
                        .min(ptrs_per_block * ppb2);
                    let trel_to = (old_num_blocks - tind_zone_start).min(ptrs_per_block * ppb2);

                    let dind_from = trel_from / ppb2;
                    let dind_to = (trel_to + ppb2 - 1) / ppb2;

                    let mut tind_buf = vec![0u8; bsz];
                    self.read_block(tind_ptr_blk, &mut tind_buf)?;
                    let mut tind_dirty = false;

                    for dind_i in dind_from..dind_to {
                        let dind_ptr_blk = read_ptr(&tind_buf, dind_i);
                        if dind_ptr_blk != 0 {
                            // Relative range within this dind's address space.
                            let drel_from = trel_from.saturating_sub(dind_i * ppb2).min(ppb2);
                            let drel_to = trel_to.min((dind_i + 1) * ppb2) - dind_i * ppb2;

                            let ind_from = drel_from / ptrs_per_block;
                            let ind_to = (drel_to + ptrs_per_block - 1) / ptrs_per_block;

                            let mut dind_buf = vec![0u8; bsz];
                            self.read_block(dind_ptr_blk, &mut dind_buf)?;
                            let mut dind_dirty = false;

                            for ind_i in ind_from..ind_to {
                                let ind_ptr_blk = read_ptr(&dind_buf, ind_i);
                                if ind_ptr_blk != 0 {
                                    let blk_start = if ind_i == ind_from {
                                        drel_from % ptrs_per_block
                                    }
                                    else {
                                        0
                                    };
                                    let blk_end = {
                                        let abs_end = drel_to.min((ind_i + 1) * ptrs_per_block);
                                        abs_end - ind_i * ptrs_per_block
                                    };

                                    let mut ind_buf = vec![0u8; bsz];
                                    self.read_block(ind_ptr_blk, &mut ind_buf)?;
                                    for j in blk_start..blk_end {
                                        let blk = read_ptr(&ind_buf, j);
                                        if blk != 0 {
                                            let _ = self.get_superblock_mut().deallocate_block(blk);
                                            write_ptr(&mut ind_buf, j, 0);
                                        }
                                    }
                                    self.write_block(ind_ptr_blk, &ind_buf)?;

                                    if blk_start == 0 {
                                        let _ =
                                            self.get_superblock_mut().deallocate_block(ind_ptr_blk);
                                        write_ptr(&mut dind_buf, ind_i, 0);
                                        dind_dirty = true;
                                    }
                                }
                            }

                            if dind_dirty {
                                self.write_block(dind_ptr_blk, &dind_buf)?;
                            }

                            // Free the dind pointer block if we cleared it from the start.
                            if drel_from == 0 {
                                let _ = self.get_superblock_mut().deallocate_block(dind_ptr_blk);
                                write_ptr(&mut tind_buf, dind_i, 0);
                                tind_dirty = true;
                            }
                        }
                    }

                    if tind_dirty {
                        self.write_block(tind_ptr_blk, &tind_buf)?;
                    }

                    // Free the tind pointer block if no dind entries remain.
                    if new_num_blocks <= tind_zone_start {
                        let _ = self.get_superblock_mut().deallocate_block(tind_ptr_blk);
                        let mut blocks = self.get_inode(inode_num)?.get_blocks();
                        blocks[consts::TINDIRECT_ZONE as usize] = 0;
                        self.get_inode_mut(inode_num)?.set_blocks(&blocks);
                    }
                }
            }
        }

        self.get_inode_mut(inode_num)?.set_size(size);
        Ok(size)
    }

    fn write_symlink(&mut self, inode: u64, target: &String) -> io::Result<u64> {
        let target_bytes = target.len();
        if target_bytes > consts::MAX_SYMLINK_SIZE {
            return Err(Error::new(ErrorKind::FileTooLarge, "symlink too large"));
        }
        if self.get_inode(inode)?.get_blocks().iter().all(|&x| x == 0) {
            self.truncate(inode, target_bytes as u64)?;
        }
        let blocks = self.get_inode(inode)?.get_blocks();
        if blocks.is_empty() {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "could not create symlink block",
            ));
        }
        self.write_block(blocks[0], target.as_bytes())?;
        self.get_inode_mut(inode)?.set_size(target_bytes as u64);
        Ok(target_bytes as u64)
    }

    fn read_symlink(&mut self, inode: u64) -> io::Result<String> {
        let mut data: Vec<u8> = vec![];
        data.resize(consts::MAX_SYMLINK_SIZE, 0);
        let size = match self.read_file(inode, 0, &mut data) {
            Ok(x) => x as usize,
            Err(x) => return Err(x),
        };
        Ok(String::from_utf8_lossy(&data[..size]).to_string())
    }

    fn unlink(&mut self, abs_path: &String) -> io::Result<()> {
        let split = split_items(self, &abs_path)?;
        let fname = fname_from_path(abs_path);
        let dent_inode_num = split.dir_part.inode();
        let dent_inode = self.get_inode(dent_inode_num)?;
        let dent_blocks = dent_inode.get_blocks();
        let mut block_data = Vec::new();
        block_data.resize(self.get_block_size() as usize, 0);
        let mut dent_found = false;
        for i in 0..INDIRECT_ZONE {
            let zone = dent_blocks[i];
            if zone == 0 {
                continue;
            }
            self.read_block(zone, &mut block_data)?;
            for d in (0..self.get_block_size() as usize).step_by(DIR_ENTRY_BYTES as usize) {
                if let Some(dent) = DirEntry::from_bytes_mut(&mut block_data[d..]) {
                    if dent.name() == fname {
                        dent.inode = 0;
                        self.bcache.insert_write(zone, block_data.clone());
                        remove_item(self, abs_path)?;
                        dent_found = true;
                        break;
                    }
                }
            }
        }
        if !dent_found {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("{}: No such file or directory.", abs_path),
            ));
        }
        // Grabbing the inode borrows it mutably, so we have to drop after we
        // get the information we need, which is after decrementing the link counter.
        let (fl_nlinks, fl_inode_num) = {
            let fl_inode_num = split.file_part.inode();
            let fl_inode = self.get_inode_mut(fl_inode_num)?;
            let fl_nlinks = fl_inode.get_nlinks().saturating_sub(1);
            fl_inode.set_nlinks(fl_nlinks);
            (fl_nlinks, fl_inode_num)
        };
        // If the number of links is not 0, we can't free the inode or blocks.
        if fl_nlinks > 0 {
            // Decrease the size of the directory by the size of one directory entry.
            let dir_entry_size = DIR_ENTRY_BYTES as usize;
            let inode = self.get_inode_mut(dent_inode_num)?;
            let size = inode.get_size();
            inode.set_size(size.saturating_sub(dir_entry_size as u64));
            // Don't continue below because that frees the inode and blocks, but in here,
            // the link count is not 0, so it cannot be freed.
            return Ok(());
        }
        // If we get here, number of links is 0, so we can free the inode and all its blocks.

        // Free the blocks by calling truncate.
        self.truncate(fl_inode_num, 0)?;

        let ftype = {
            // Clear the inode. We technically don't need to do this, but it makes it obvious
            // the inode is free.
            let inode = self.get_inode_mut(fl_inode_num)?;
            let ftype = inode.get_file_type();
            inode.set_file_type(FileType::Invalid);
            inode.set_size(0);
            inode.set_mode(0);
            inode.set_atime(0);
            inode.set_mtime(0);
            inode.set_ctime(0);
            // inode.set_blocks(&[0; consts::NUM_ZONE_POINTERS as usize]);
            ftype
        };

        {
            // Decrease the size of the directory by the size of one directory entry.
            let parent = split.dir_part.inode();
            let parent_inode = self.get_inode_mut(parent)?;
            let parent_size = parent_inode.get_size();
            let new_parent_size = parent_size.saturating_sub(DIR_ENTRY_BYTES as u64);
            parent_inode.set_size(new_parent_size);

            // If we are removing a directory, we need to decrease the link count of the parent
            // by 1 to account for the removed directory's ".." entry.
            if ftype == FileType::Directory {
                let nlinks = parent_inode.get_nlinks().saturating_sub(1);
                parent_inode.set_nlinks(nlinks);
            }
        }

        // Deallocate the inode by clearing the bitmap.
        let _ = self.get_superblock_mut().deallocate_inode(fl_inode_num);

        // All done!
        Ok(())
    }

    /// Create a directory entry that links the given inode with the given path.
    /// The path must be absolute.
    fn link(&mut self, inode: u64, abs_path: &String) -> io::Result<u64> {
        create::create_dentry(self, inode, abs_path)
    }

    /// Create an entry in the parent directory of `abs_path` that links to `inode`.
    /// The path must be absolute.
    fn create(&mut self, abs_path: &String, ftype: FileType) -> io::Result<u64> {
        create::create(self, abs_path, ftype)
    }

    fn get_inode(&mut self, inode_num: u64) -> io::Result<&'_ dyn InodeOperations> {
        if inode_num == 0 || inode_num > self.superblock.num_inodes as u64 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Invalid inode number: {}", inode_num),
            ));
        }
        Ok(&self.inodes[(inode_num - 1) as usize])
    }

    fn get_inode_mut(&mut self, inode_num: u64) -> io::Result<&'_ mut dyn InodeOperations> {
        if inode_num == 0 || inode_num > self.superblock.num_inodes as u64 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Invalid inode number: {}", inode_num),
            ));
        }
        Ok(&mut self.inodes[(inode_num - 1) as usize])
    }

    fn get_tree(&self) -> Option<&Tree> {
        Some(&self.tree)
    }

    fn get_tree_mut(&mut self) -> Option<&mut Tree> {
        Some(&mut self.tree)
    }

    fn get_superblock(&self) -> &dyn SuperblockOperations {
        self
    }

    fn get_superblock_mut(&mut self) -> &mut dyn SuperblockOperations {
        self
    }

    fn write_to_backing(&mut self) -> io::Result<()> {
        self.stream
            .seek(SeekFrom::Start(self.superblock.block_size as u64 * 2))?;
        self.stream.write_all(self.imap.get_map())?;
        self.stream.seek(SeekFrom::Start(
            self.superblock.block_size as u64 * (2 + self.superblock.imap_blocks as u64),
        ))?;
        self.stream.write_all(self.zmap.get_map())?;
        self.stream.seek(SeekFrom::Start(
            self.superblock.block_size as u64
                * (2 + self.superblock.imap_blocks as u64 + self.superblock.zmap_blocks as u64),
        ))?;
        self.inodes
            .iter()
            .try_for_each(|inode| self.stream.write_all(inode.as_bytes()))?;
        self.write_back_dirty_blocks()?;
        self.bcache.clean_all();
        self.bcache.clear();
        Ok(())
    }
}
