use hash_roll::{fastcdc, gear_table::GEAR_64, ChunkIncr};
use memmap::Mmap;

use super::error::*;
use super::traits::*;
use super::structs::*;

/// Calculates the $log_2$ on an u64
pub fn log2u64(x: u64) -> Option<u64> {
    if x > 0 {
        Some(std::mem::size_of::<u64>() as u64 * 8u64 - x.leading_zeros() as u64 - 1u64)
    } else {
        None
    }
}


/// Calculate chunks, chunk hashes and a file-hash of mmaped data.
/// Returns a Vec of `(Chunk, ChunkHash)` tuples and the FileHash.
pub fn chunk_and_hash(
    mmap: &Mmap,
    conf: &ChunkerConf,
    chunk_hash_key: &EncKey,
    file_hash_key: &EncKey,
) -> Result<(Vec<(Vec<u8>, blake3::Hash)>, blake3::Hash)> {
    let cdc = fastcdc::FastCdc::new(
        &GEAR_64,
        conf.minimum_chunk_size,
        conf.average_chunk_size,
        conf.maximum_chunk_size,
    );
    let chunk_iter = fastcdc::FastCdcIncr::from(&cdc);

    let chunks: Vec<(Vec<u8>, blake3::Hash)> = chunk_iter
        .iter_slices(&mmap[..])
        .map(|chunk| {
            (
                chunk.to_vec(),
                chunk.keyed_hash(chunk_hash_key).expect("never fail"),
            )
        })
        .collect();

    Ok((chunks, (&mmap[..]).keyed_hash(file_hash_key)?))
}
