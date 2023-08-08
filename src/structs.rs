use chacha20poly1305::aead::{rand_core::RngCore, OsRng};
use generic_array::GenericArray;
use hash_roll::{fastcdc, gear_table::GEAR_64, ChunkIncr};
use memmap::Mmap;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use typenum::{
    bit::{B0, B1},
    uint::{UInt, UTerm},
};

use super::error::*;
use super::traits::*;
use super::db::*;

pub const SALT_SIZE: usize = 32;
pub const HASH_SIZE: usize = 32;
pub const KEY_SIZE: usize = 32;
pub const NONCE_SIZE: usize = 24;
pub const CRYPTO_KEYS_SIZE: usize = KEY_SIZE * 4;

// Signature key for manifest + encrypted keys for data
pub const TOTAL_KEY_SIZE: usize = KEY_SIZE + CRYPTO_KEYS_SIZE;

pub type RefCount = usize;

#[derive(
    Debug, Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Hash, Default, Serialize, Deserialize,
)]
pub struct Hash([u8; HASH_SIZE]);

impl From<[u8; HASH_SIZE]> for Hash {
    fn from(array: [u8; HASH_SIZE]) -> Self {
        Hash(array)
    }
}

impl TryFrom<&[u8]> for Hash {
    type Error = std::array::TryFromSliceError;
    fn try_from(array: &[u8]) -> std::result::Result<Self, std::array::TryFromSliceError> {
        Ok(Hash(<[u8; HASH_SIZE]>::try_from(array)?))
    }
}

impl AsMut<[u8]> for Hash {
    fn as_mut(&mut self) -> &mut [u8] {
        self.0.as_mut()
    }
}

impl AsRef<[u8]> for Hash {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

#[derive(
    Debug, Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Hash, Default, Serialize, Deserialize,
)]
pub struct ChunkHash([u8; HASH_SIZE]);

impl From<[u8; HASH_SIZE]> for ChunkHash {
    fn from(array: [u8; HASH_SIZE]) -> Self {
        ChunkHash(array)
    }
}

impl TryFrom<&[u8]> for ChunkHash {
    type Error = std::array::TryFromSliceError;
    fn try_from(array: &[u8]) -> std::result::Result<Self, std::array::TryFromSliceError> {
        Ok(ChunkHash(<[u8; HASH_SIZE]>::try_from(array)?))
    }
}

impl AsMut<[u8]> for ChunkHash {
    fn as_mut(&mut self) -> &mut [u8] {
        self.0.as_mut()
    }
}

impl AsRef<[u8]> for ChunkHash {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

#[derive(
    Debug, Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Hash, Default, Serialize, Deserialize,
)]
pub struct InodeHash([u8; HASH_SIZE]);

impl From<[u8; HASH_SIZE]> for InodeHash {
    fn from(array: [u8; HASH_SIZE]) -> Self {
        InodeHash(array)
    }
}

impl TryFrom<&[u8]> for InodeHash {
    type Error = std::array::TryFromSliceError;
    fn try_from(array: &[u8]) -> std::result::Result<Self, std::array::TryFromSliceError> {
        Ok(InodeHash(<[u8; HASH_SIZE]>::try_from(array)?))
    }
}

impl AsMut<[u8]> for InodeHash {
    fn as_mut(&mut self) -> &mut [u8] {
        self.0.as_mut()
    }
}

impl AsRef<[u8]> for InodeHash {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

#[derive(
    Debug, Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Hash, Default, Serialize, Deserialize,
)]
pub struct EncKey([u8; KEY_SIZE]);

impl From<GenericArray<u8, UInt<UInt<UInt<UInt<UInt<UInt<UTerm, B1>, B0>, B0>, B0>, B0>, B0>>>
    for EncKey
{
    fn from(
        array: GenericArray<u8, UInt<UInt<UInt<UInt<UInt<UInt<UTerm, B1>, B0>, B0>, B0>, B0>, B0>>,
    ) -> Self {
        EncKey(<[u8; KEY_SIZE]>::from(array))
    }
}

impl EncKey {
    pub fn as_array(&self) -> &[u8; KEY_SIZE] {
        &self.0
    }

    pub fn xor_keys(&self, key: &EncKey) -> EncKey {
        let l = self.as_array();
        let r = key.as_array();
        let l_iter = l.iter();
        let r_iter = r.iter();
        let result: Vec<u8> = l_iter.zip(r_iter).map(|(l, r)| l ^ r).collect();
        let result: [u8; KEY_SIZE] = result
            .try_into()
            .expect("This can never fail because all length match");
        EncKey::from(result)
    }
}

impl From<[u8; KEY_SIZE]> for EncKey {
    fn from(array: [u8; KEY_SIZE]) -> Self {
        EncKey(array)
    }
}

impl TryFrom<&[u8]> for EncKey {
    type Error = std::array::TryFromSliceError;
    fn try_from(array: &[u8]) -> std::result::Result<Self, std::array::TryFromSliceError> {
        Ok(EncKey(<[u8; KEY_SIZE]>::try_from(array)?))
    }
}

impl AsMut<[u8]> for EncKey {
    fn as_mut(&mut self) -> &mut [u8] {
        self.0.as_mut()
    }
}

impl AsRef<[u8]> for EncKey {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

#[derive(
    Debug, Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Hash, Default, Serialize, Deserialize,
)]
pub struct EncNonce([u8; NONCE_SIZE]);

impl From<GenericArray<u8, UInt<UInt<UInt<UInt<UInt<UTerm, B1>, B1>, B0>, B0>, B0>>> for EncNonce {
    fn from(
        array: GenericArray<u8, UInt<UInt<UInt<UInt<UInt<UTerm, B1>, B1>, B0>, B0>, B0>>,
    ) -> Self {
        EncNonce(<[u8; NONCE_SIZE]>::from(array))
    }
}

impl EncNonce {
    pub fn as_array(&self) -> &[u8; NONCE_SIZE] {
        &self.0
    }
}

impl From<[u8; NONCE_SIZE]> for EncNonce {
    fn from(array: [u8; NONCE_SIZE]) -> Self {
        EncNonce(array)
    }
}

impl TryFrom<&[u8]> for EncNonce {
    type Error = std::array::TryFromSliceError;
    fn try_from(array: &[u8]) -> std::result::Result<Self, std::array::TryFromSliceError> {
        Ok(EncNonce(<[u8; NONCE_SIZE]>::try_from(array)?))
    }
}

impl AsMut<[u8]> for EncNonce {
    fn as_mut(&mut self) -> &mut [u8] {
        self.0.as_mut()
    }
}

impl AsRef<[u8]> for EncNonce {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

#[derive(
    Debug, Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Hash, Default, Serialize, Deserialize,
)]
pub struct SigKey([u8; KEY_SIZE]);

impl From<GenericArray<u8, UInt<UInt<UInt<UInt<UInt<UInt<UTerm, B1>, B0>, B0>, B0>, B0>, B0>>>
    for SigKey
{
    fn from(
        array: GenericArray<u8, UInt<UInt<UInt<UInt<UInt<UInt<UTerm, B1>, B0>, B0>, B0>, B0>, B0>>,
    ) -> Self {
        SigKey(<[u8; KEY_SIZE]>::from(array))
    }
}

impl SigKey {
    pub fn as_array(&self) -> &[u8; KEY_SIZE] {
        &self.0
    }
}

impl From<[u8; KEY_SIZE]> for SigKey {
    fn from(array: [u8; KEY_SIZE]) -> Self {
        SigKey(array)
    }
}

impl TryFrom<&[u8]> for SigKey {
    type Error = std::array::TryFromSliceError;
    fn try_from(array: &[u8]) -> std::result::Result<Self, std::array::TryFromSliceError> {
        Ok(SigKey(<[u8; KEY_SIZE]>::try_from(array)?))
    }
}

impl AsMut<[u8]> for SigKey {
    fn as_mut(&mut self) -> &mut [u8] {
        self.0.as_mut()
    }
}

impl AsRef<[u8]> for SigKey {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

//type BackupHash = [u8; HASH_SIZE];

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BackupConf {
    follow_symlinks: bool,
}

impl Default for BackupConf {
    fn default() -> Self {
        BackupConf {
            follow_symlinks: false,
        }
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
/*
#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeConf {
    manifest_sig_key: SigKey,
    chunk_encryption_key: EncKey,
    chunk_hash_key: EncKey,
    inode_encryption_key: EncKey,
    inode_hash_key: EncKey,
    backup_encryption_key: EncKey,
}
*/
#[derive(Clone, Hash, Copy, Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Metadata {
    mode: u32,
    uid: u32,
    gid: u32,
    mtime: i64,
    mtime_ns: i64,
    ctime: i64,
    ctime_ns: i64,
}

#[derive(Clone, Hash, Copy, Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChunkerConf {
    pub minimum_chunk_size: u64,
    pub average_chunk_size: u64,
    pub maximum_chunk_size: u64,
}

#[derive(Clone, Hash, Copy, Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EncCryptoKeys {
    enc_chunk_hash_key: EncKey,
    enc_chunk_enc_key: EncKey,
    enc_inode_hash_key: EncKey,
    enc_inode_enc_key: EncKey,
}

impl EncCryptoKeys {
    pub fn decrypt(&self, keys: KeyEncryptionKeys) -> CryptoKeys {
        CryptoKeys {
            chunk_hash_key: self.enc_chunk_hash_key.xor_keys(&keys.key_chunk_hash_key),
            chunk_enc_key: self.enc_chunk_enc_key.xor_keys(&keys.key_chunk_enc_key),
            inode_hash_key: self.enc_inode_hash_key.xor_keys(&keys.key_inode_hash_key),
            inode_enc_key: self.enc_inode_enc_key.xor_keys(&keys.key_inode_enc_key),
        }
    }
}

#[derive(Clone, Hash, Copy, Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct KeyEncryptionKeys {
    pub key_chunk_hash_key: EncKey,
    pub key_chunk_enc_key: EncKey,
    pub key_inode_hash_key: EncKey,
    pub key_inode_enc_key: EncKey,
}

impl From<[u8; CRYPTO_KEYS_SIZE]> for KeyEncryptionKeys {
    fn from(keys: [u8; CRYPTO_KEYS_SIZE]) -> Self {
        let mut n = KEY_SIZE;
        let key_chunk_hash_key = EncKey::try_from(&keys[n - KEY_SIZE..n])
            .expect("This can not fail because we take care of the correct size here");
        n += KEY_SIZE;
        let key_chunk_enc_key = EncKey::try_from(&keys[n - KEY_SIZE..n])
            .expect("This can not fail because we take care of the correct size here");
        n += KEY_SIZE;
        let key_inode_hash_key = EncKey::try_from(&keys[n - KEY_SIZE..n])
            .expect("This can not fail because we take care of the correct size here");
        n += KEY_SIZE;
        let key_inode_enc_key = EncKey::try_from(&keys[n - KEY_SIZE..n])
            .expect("This can not fail because we take care of the correct size here");

        KeyEncryptionKeys {
            key_chunk_hash_key,
            key_chunk_enc_key,
            key_inode_hash_key,
            key_inode_enc_key,
        }
    }
}

#[derive(Clone, Hash, Copy, Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CryptoKeys {
    pub chunk_hash_key: EncKey,
    pub chunk_enc_key: EncKey,
    pub inode_hash_key: EncKey,
    pub inode_enc_key: EncKey,
}

impl CryptoKeys {
    pub fn new() -> Self {
        let mut keys = [0u8; CRYPTO_KEYS_SIZE];
        OsRng.fill_bytes(&mut keys);
        let mut n = KEY_SIZE;
        let chunk_hash_key = EncKey::try_from(&keys[n - KEY_SIZE..n])
            .expect("This can not fail because we take care of the correct size here");
        n += KEY_SIZE;
        let chunk_enc_key = EncKey::try_from(&keys[n - KEY_SIZE..n])
            .expect("This can not fail because we take care of the correct size here");
        n += KEY_SIZE;
        let inode_hash_key = EncKey::try_from(&keys[n - KEY_SIZE..n])
            .expect("This can not fail because we take care of the correct size here");
        n += KEY_SIZE;
        let inode_enc_key = EncKey::try_from(&keys[n - KEY_SIZE..n])
            .expect("This can not fail because we take care of the correct size here");

        CryptoKeys {
            chunk_hash_key,
            chunk_enc_key,
            inode_hash_key,
            inode_enc_key,
        }
    }
}

impl CryptoKeys {
    pub fn encrypt(&self, keys: KeyEncryptionKeys) -> EncCryptoKeys {
        EncCryptoKeys {
            enc_chunk_hash_key: self.chunk_hash_key.xor_keys(&keys.key_chunk_hash_key),
            enc_chunk_enc_key: self.chunk_enc_key.xor_keys(&keys.key_chunk_enc_key),
            enc_inode_hash_key: self.inode_hash_key.xor_keys(&keys.key_inode_hash_key),
            enc_inode_enc_key: self.inode_enc_key.xor_keys(&keys.key_inode_enc_key),
        }
    }
}

#[derive(Clone, Hash, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Argon2Conf {
    pub threads: u32,
    pub mem_cost: u32,
    pub time_cost: u32,
    pub variant: u32,
    pub version: u32,
}

impl Default for Argon2Conf {
    fn default() -> Self {
        Argon2Conf {
            threads: 4,
            mem_cost: 1024 * 1024 * 2, // 2 GB per thread => 8GB total
            time_cost: 20,             // very conservative value
            variant: argon2::Variant::Argon2id.as_u32(),
            version: argon2::Version::Version13.as_u32(),
        }
    }
}

impl Argon2Conf {
    pub fn as_argon2config(&self) -> Result<argon2::Config> {
        Ok(argon2::Config {
            ad: &[],
            hash_length: TOTAL_KEY_SIZE as u32,
            lanes: self.threads,
            mem_cost: self.mem_cost,
            secret: &[],
            thread_mode: argon2::ThreadMode::from_threads(self.threads),
            time_cost: self.time_cost,
            variant: argon2::Variant::from_u32(self.variant)?,
            version: argon2::Version::from_u32(self.version)?,
        })
    }
}

#[derive(Clone, Hash, Debug, Serialize, Deserialize)]
pub struct SignedManifest {
    manifest: Manifest,
    signature: [u8; 32],
}

impl SignedManifest {
    pub fn verify(&self, key: &EncKey) -> Result<Manifest> {
        let self_ = (*self).clone();
        let verify = *self.manifest.keyed_hash(key)?.as_bytes();
        if verify != self_.signature {
            Err(BackrubError::InvalidSignature.into())
        } else {
            Ok(self_.manifest)
        }
    }
    pub fn get_salt(&self) -> [u8; SALT_SIZE] {
        self.manifest.salt
    }

    pub fn get_argon2config(&self) -> Result<argon2::Config> {
        self.manifest.argon2_conf.as_argon2config()
    }
}

#[derive(Clone, Hash, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Manifest {
    pub salt: [u8; SALT_SIZE],
    pub chunk_root_dir: PathBuf,
    pub db_path: PathBuf,
    pub version: String,
    pub chunker_conf: ChunkerConf,
    pub keys: EncCryptoKeys,
    //completed_backups: BTreeMap<BackupHash,Vec<u8>>,
    pub argon2_conf: Argon2Conf,
    pub chunk_db_state: ChunkDbState,
}

impl Hashable for Manifest {}

impl Manifest {
    //        pub fn new()

    pub fn sign(&self, key: &EncKey) -> Result<SignedManifest> {
        let manifest = (*self).clone();
        let signature = *self.keyed_hash(key)?.as_bytes();
        Ok(SignedManifest {
            manifest,
            signature,
        })
    }
}

#[derive(Clone, Hash, Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Backup {
    timestamp: String,
    name: String,
    root: InodeHash,
}

#[derive(Clone, Hash, Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Symlink {
    relpath: PathBuf,
    target: PathBuf,
    metadata: Metadata,
}

#[derive(Clone, Hash, Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Directory {
    relpath: PathBuf,
    metadata: Metadata,
    contents: Vec<InodeHash>,
}

#[derive(Clone, Hash, Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct File {
    relpath: PathBuf,
    chunk_ids: Vec<ChunkHash>,
    metadata: Metadata,
}

#[derive(Clone, Hash, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Inode {
    File(File),
    Directory(Directory),
    Symlink(Symlink),
}

impl Hashable for Inode {}

#[derive(Clone, Hash, Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct Chunk {
    data: Vec<u8>,
}

impl Encrypt for Chunk {}
impl Hashable for Chunk {}

pub fn log2u64(x: u64) -> Option<u64> {
    if x > 0 {
        Some(std::mem::size_of::<u64>() as u64 * 8u64 - x.leading_zeros() as u64 - 1u64)
    } else {
        None
    }
}

#[derive(Clone, Hash, Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct FilePathGen(u64);

impl From<u64> for FilePathGen {
    fn from(value: u64) -> Self {
        FilePathGen { 0: value }
    }
}

impl Iterator for FilePathGen {
    type Item = String;

    fn next(&mut self) -> std::option::Option<<Self as Iterator>::Item> {
        if self.0 < std::u64::MAX {
            self.0 += 1;
            let mut name = String::new();
            // we need floor(log2(self.0)/8) folders and 1 byte for the file_name
            //folders are
            let num_bytes = log2u64(self.0)? / 8u64 + 1u64;
            for i in 1..num_bytes {
                // b0 = 0xff
                let mut b0 = !0u8 as u64;
                // shift b0 in position
                b0 = b0 << 8 * i;
                // apply mask
                b0 = self.0 & b0;
                // shift back
                let b0 = (b0 >> (8 * i)) as u8;
                // add folder
                name += &format!("{b0:x}");
                name += "/";
            }

            let mut b0 = !0u8 as u64;
            b0 = self.0 & b0;
            let b0 = b0 as u8;
            name += &format!("{b0:x}.bin");
            Some(name)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn test_compressed_encryption_success() {
        let mut data = Vec::<u8>::new();
        for n in 0..1024 * 1024 {
            for i in 0..5 {
                data.push(i);
            }
        }
        let testdata = Chunk { data };
        let key = XChaCha20Poly1305::generate_key(&mut OsRng);

        let now = Instant::now();

        let enc = testdata.compress_and_encrypt(&key.into()).unwrap();
        let dec = Chunk::decrypt_and_uncompress(&enc, &key.into()).unwrap();

        let elapsed = now.elapsed();

        println!("0: size data uncompressed: {}", testdata.data.len());
        println!("0: size data compressed and encrypted: {}", enc.len());
        println!("0: runtime in nanoseconds: {}", elapsed.as_nanos());
        assert_eq!(testdata, dec);
    }

    #[test]
    fn test_compressed_encryption_fail_tempering() {
        let mut data = Vec::<u8>::new();
        for n in 0..1024 * 1024 {
            for i in 0..5 {
                data.push(i);
            }
        }
        let testdata = Chunk { data };

        let key = XChaCha20Poly1305::generate_key(&mut OsRng);
        let mut enc = testdata.compress_and_encrypt(&key.into()).unwrap();
        let last = enc.pop().unwrap();
        enc.push(!last);

        let dec = Chunk::decrypt_and_uncompress(&enc, &key.into());
        assert!(dec.is_err());
    }

    #[test]
    fn test_compressed_encryption_fail_key() {
        let mut data = Vec::<u8>::new();
        for n in 0..1024 * 1024 {
            for i in 0..5 {
                data.push(i);
            }
        }
        let testdata = Chunk { data };

        let mut key = XChaCha20Poly1305::generate_key(&mut OsRng);
        let enc = testdata.compress_and_encrypt(&key.into()).unwrap();
        key[0] = !key[0];

        let dec = Chunk::decrypt_and_uncompress(&enc, &key.into());
        assert!(dec.is_err());
    }

    #[test]
    fn test_encryption_success() {
        let mut data = Vec::<u8>::new();
        for n in 0..1024 * 1024 {
            for i in 0..5 {
                data.push(i);
            }
        }
        let testdata = Chunk { data };
        let key = XChaCha20Poly1305::generate_key(&mut OsRng);

        let now = Instant::now();

        let enc = testdata.encrypt(&key.into()).unwrap();
        let dec = Chunk::decrypt(&enc, &key.into()).unwrap();

        let elapsed = now.elapsed();

        println!("1: size data uncompressed: {}", testdata.data.len());
        println!("1: size data encrypted: {}", enc.len());
        println!("1: runtime in nanoseconds: {}", elapsed.as_nanos());
        assert_eq!(testdata, dec);
    }

    #[test]
    fn test_encryption_fail_tempering() {
        let mut data = Vec::<u8>::new();
        for n in 0..1024 * 1024 {
            for i in 0..5 {
                data.push(i);
            }
        }
        let testdata = Chunk { data };
        let key = XChaCha20Poly1305::generate_key(&mut OsRng);
        let mut enc = testdata.encrypt(&key.into()).unwrap();
        let last = enc.pop().unwrap();
        enc.push(!last);

        let dec = Chunk::decrypt(&enc, &key.into());
        assert!(dec.is_err());
    }

    #[test]
    fn test_encryption_fail_key() {
        let mut data = Vec::<u8>::new();
        for n in 0..1024 * 1024 {
            for i in 0..5 {
                data.push(i);
            }
        }
        let testdata = Chunk { data };
        let mut key = XChaCha20Poly1305::generate_key(&mut OsRng);
        let enc = testdata.encrypt(&key.into()).unwrap();
        key[0] = !key[0];

        let dec = Chunk::decrypt(&enc, &key.into());
        assert!(dec.is_err());
    }

    #[test]
    fn test_FilePathGen() {
        let mut s = std::collections::HashSet::<String>::new();
        let mut fg = FilePathGen::default();
        for _ in 0..256 {
            for i in 0..1024 {
                let next = fg.next().unwrap();
                s.insert(next);
            }
        }
        assert_eq!(s.len(), 256 * 1024);
        assert_eq!(fg, FilePathGen { 0: 256 * 1024 });

        let mut fg = FilePathGen::from_U64(!0u64 - 1);
        assert_eq!(fg.next(), Some(String::from("ff/ff/ff/ff/ff/ff/ff/ff.bin")));
        assert_eq!(fg.next(), None);
    }

    #[test]
    fn test_log2u64() {
        assert_eq!(log2u64(0u64), None);
        assert_eq!(log2u64(!0u64), Some(63u64));
        assert_eq!(log2u64(1u64), Some(0u64));
        assert_eq!(log2u64(2u64), Some(1u64));
        assert_eq!(log2u64(64u64), Some(6u64));
        assert_eq!(log2u64(63u64), Some(5u64));
    }

    #[test]
    fn test_ChunkDb() {
        let key = EncKey::from(*blake3::hash(b"foobar").as_bytes());
        let config = sled::Config::new().temporary(true);
        let db = config.open().unwrap();

        let mut cs = ChunkDb::new(db.open_tree(b"test").unwrap(), key).unwrap();
        let h1 = ChunkHash::from(*blake3::hash(b"foo").as_bytes());
        let h2 = ChunkHash::from(*blake3::hash(b"bar").as_bytes());
        let h3 = ChunkHash::from(*blake3::hash(b"baz").as_bytes());
        let h4 = ChunkHash::from(*blake3::hash(b"foobar").as_bytes());

        assert_eq!(cs.insert(&h1).unwrap(), (1, PathBuf::from("1.bin")));
        assert_eq!(cs.insert(&h2).unwrap(), (1, PathBuf::from("2.bin")));
        assert_eq!(cs.insert(&h3).unwrap(), (1, PathBuf::from("3.bin")));

        assert_eq!(cs.insert(&h2).unwrap(), (2, PathBuf::from("2.bin")));
        assert_eq!(cs.insert(&h3).unwrap(), (2, PathBuf::from("3.bin")));

        assert_eq!(cs.remove(&h1).unwrap(), Some((0, PathBuf::from("1.bin"))));
        assert_eq!(cs.remove(&h1).unwrap(), None);

        assert_eq!(cs.insert(&h4).unwrap(), (1, PathBuf::from("1.bin")));
    }

    #[test]
    fn test_CryptoKeys_encryption() {
        let ck = CryptoKeys::new();

        let mut raw_keys = [0u8; CRYPTO_KEYS_SIZE];
        OsRng.fill_bytes(&mut raw_keys);
        let kek = KeyEncryptionKeys::from(raw_keys);

        let enc_keys = ck.encrypt(kek.clone());
        let dec_keys = enc_keys.decrypt(kek);

        assert_eq!(ck, dec_keys);
    }
}
