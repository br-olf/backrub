use chacha20poly1305::aead::{rand_core::RngCore, OsRng};
use generic_array::GenericArray;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use typenum::{
    bit::{B0, B1},
    uint::{UInt, UTerm},
};

use super::db::*;
use super::error::*;
use super::traits::*;
use super::utils::*;

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
pub struct Chunk {
    pub data: Vec<u8>,
}

impl Encrypt for Chunk {}
impl Hashable for Chunk {}

#[derive(Clone, Hash, Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct FilePathGen(pub(crate) u64);

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
