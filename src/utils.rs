use hash_roll::{fastcdc, gear_table::GEAR_64, ChunkIncr};
use memmap::Mmap;

use super::error::*;
use super::structs::*;
use super::traits::*;

/// Calculates the log2 on an [u64]
pub fn log2u64(x: u64) -> Option<u64> {
    match x {
        0 => None,
        _ => Some(std::mem::size_of::<u64>() as u64 * 8u64 - x.leading_zeros() as u64 - 1u64),
    }
}

/// Calculate chunks, chunk hashes and a file-hash of mmaped data.
/// Returns a [Vec] of `(Chunk, ChunkHash)` tuples and the FileHash.
use std::sync::Arc;
pub fn chunk_and_hash(
    mmap: &Mmap,
    conf: &ChunkerConf,
    chunk_hash_key: &Key256,
    file_hash_key: &Key256,
) -> Result<(Arc<[(Arc<[u8]>, blake3::Hash)]>, blake3::Hash)> {
    let cdc = fastcdc::FastCdc::new(
        &GEAR_64,
        conf.minimum_chunk_size,
        conf.average_chunk_size,
        conf.maximum_chunk_size,
    );
    let chunk_iter = fastcdc::FastCdcIncr::from(&cdc);

    let chunks: Vec<(Arc<[u8]>, blake3::Hash)> = chunk_iter
        .iter_slices(&mmap[..])
        .map(|chunk| {
            (
                Arc::<[u8]>::from(chunk),
                chunk.keyed_hash(chunk_hash_key).expect("never fail"),
            )
        })
        .collect();

    Ok((chunks.into(), (&mmap[..]).keyed_hash(file_hash_key)?))
}
