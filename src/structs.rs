use async_std::sync::Mutex;
use chacha20poly1305::{
    aead::{rand_core::RngCore, Aead, AeadCore, KeyInit, OsRng},
    XChaCha20Poly1305,
};
use flate2::write::{DeflateDecoder, DeflateEncoder};
use flate2::Compression;
use futures::executor::block_on;
use generic_array::GenericArray;
use hash_roll::{fastcdc, gear_table::GEAR_64, ChunkIncr};
use memmap::Mmap;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::prelude::*;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use typenum::{
    bit::{B0, B1},
    uint::{UInt, UTerm},
};

const SALT_SIZE: usize = 32;
const HASH_SIZE: usize = 32;
const KEY_SIZE: usize = 32;
const NONCE_SIZE: usize = 24;
const CRYPTO_KEYS_SIZE: usize = KEY_SIZE * 4;

// Signature key for manifest + encrypted keys for data
const TOTAL_KEY_SIZE: usize = KEY_SIZE + CRYPTO_KEYS_SIZE;

type RefCount = usize;

pub trait Hashable: Serialize {
    fn hash(&self) -> Result<blake3::Hash> {
        let serialized = bincode::serialize(self)?;
        serialized.hash()
    }

    fn keyed_hash(&self, key: &EncKey) -> Result<blake3::Hash> {
        let serialized = bincode::serialize(self)?;
        serialized.keyed_hash(key)
    }
}

impl Hashable for Vec<u8> {
    /// This will never return Err
    fn hash(&self) -> Result<blake3::Hash> {
        let mut hasher = blake3::Hasher::new();
        hasher.update_rayon(&self);
        Ok(hasher.finalize())
    }

    /// This will never return Err
    fn keyed_hash(&self, key: &EncKey) -> Result<blake3::Hash> {
        let mut hasher = blake3::Hasher::new_keyed(key.as_array());
        hasher.update_rayon(&self);
        Ok(hasher.finalize())
    }
}

impl Hashable for &[u8] {
    /// This will never return Err
    fn hash(&self) -> Result<blake3::Hash> {
        let mut hasher = blake3::Hasher::new();
        hasher.update_rayon(&self);
        Ok(hasher.finalize())
    }

    /// This will never return Err
    fn keyed_hash(&self, key: &EncKey) -> Result<blake3::Hash> {
        let mut hasher = blake3::Hasher::new_keyed(key.as_array());
        hasher.update_rayon(&self);
        Ok(hasher.finalize())
    }
}

pub trait Encrypt: Serialize + for<'a> Deserialize<'a> {
    /// Generic function to encrypt data in backrub
    fn encrypt(&self, key: &EncKey) -> Result<Vec<u8>> {
        // convert data to Vec<u8>
        let serialized_data = bincode::serialize(self)?;
        serialized_data.encrypt(key)
    }

    /// Generic function to decrypt data encrypted by backrub
    fn decrypt(data: &[u8], key: &EncKey) -> Result<Self> {
        // decrypt the data
        let data = Vec::<u8>::decrypt(data, key)?;
        // convert decrypted data to the target data type
        Ok(bincode::deserialize(&data)?)
    }

    /// Generic function to compress and encrypt data in backrub
    fn compress_and_encrypt(&self, key: &EncKey) -> Result<Vec<u8>> {
        // convert data to Vec<u8>
        let serialized_data = bincode::serialize(self)?;
        serialized_data.compress_and_encrypt(key)
    }

    /// Generic function to decrypt and uncompress data encrypted by backrub
    fn decrypt_and_uncompress(data: &[u8], key: &EncKey) -> Result<Self> {
        // decrypt and decompress the data
        let data = Vec::<u8>::decrypt_and_uncompress(data, key)?;
        // deserialize uncompressed, decrypted data
        Ok(bincode::deserialize(&data)?)
    }
}

impl Encrypt for Vec<u8> {
    fn encrypt(&self, key: &EncKey) -> Result<Vec<u8>> {
        // generate nonce
        let nonce: EncNonce = XChaCha20Poly1305::generate_nonce(&mut OsRng).into();
        // setup the cipher
        let cipher = XChaCha20Poly1305::new(key.as_array().into());
        // encrypt the data
        let encrypted_data = cipher.encrypt(nonce.as_array().into(), &self[..])?;
        // construct CryptoCtx using the nonce and the encrypted data
        let ctx = CryptoCtx {
            nonce,
            data: encrypted_data,
        };
        // convert CryptoCtx to Vec<u8>
        Ok(bincode::serialize(&ctx)?)
    }

    fn decrypt(data: &[u8], key: &EncKey) -> Result<Self> {
        // decode encrypted data to split nonce and encrypted data
        let ctx = bincode::deserialize::<CryptoCtx>(data)?;
        // setup the cipher
        let cipher = XChaCha20Poly1305::new(key.as_array().into());
        // decrypt the data
        Ok(cipher.decrypt(ctx.nonce.as_array().into(), &ctx.data[..])?)
    }

    fn compress_and_encrypt(&self, key: &EncKey) -> Result<Vec<u8>> {
        // generate nonce
        let nonce: EncNonce = XChaCha20Poly1305::generate_nonce(&mut OsRng).into();
        // setup the cipher
        let cipher = XChaCha20Poly1305::new(key.as_array().into());

        // compress the data
        let mut compressor = DeflateEncoder::new(Vec::new(), Compression::default());
        compressor.write_all(&self[..])?;
        let compressed_data = compressor.finish()?;
        // encrypt the data
        let encrypted_data = cipher.encrypt(nonce.as_array().into(), &compressed_data[..])?;
        // construct CryptoCtx using the nonce and the encrypted data
        let ctx = CryptoCtx {
            nonce,
            data: encrypted_data,
        };
        // convert CryptoCtx to Vec<u8>
        Ok(bincode::serialize(&ctx)?)
    }

    fn decrypt_and_uncompress(data: &[u8], key: &EncKey) -> Result<Self> {
        // decode encrypted data to split nonce and encrypted data
        let ctx = bincode::deserialize::<CryptoCtx>(data)?;
        // setup the cipher
        let cipher = XChaCha20Poly1305::new(key.as_array().into());
        // decrypt the data
        let decrypted_data = cipher.decrypt(ctx.nonce.as_array().into(), &ctx.data[..])?;
        // decompress decrypted data
        let uncompressed_data = Vec::new();
        let mut deflater = DeflateDecoder::new(uncompressed_data);
        deflater.write_all(&decrypted_data)?;
        Ok(deflater.finish()?)
    }
}

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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BackupManagerConf {
    chunk_root_dir: PathBuf,
    db_path: PathBuf,
    manifest_path: PathBuf,
    chunker_conf: ChunkerConf,
    argon2_conf: Argon2Conf,
}

impl Default for BackupManagerConf {
    /// **Panics if current working directory is invalid**
    fn default() -> Self {
        let _pwd = env::current_dir().expect("current working directoy could not be determinated");

        let mut chunk_root_dir = _pwd.clone();
        chunk_root_dir.push("data");

        let mut db_path = _pwd.clone();
        db_path.push("backrub.db");

        let mut manifest_path = _pwd.clone();
        manifest_path.push("backrub.manifest");

        let chunker_conf = ChunkerConf {
            minimum_chunk_size: 4 * 1024 * 1024,
            average_chunk_size: 16 * 1024 * 1024,
            maximum_chunk_size: 64 * 1024 * 1024,
        };

        let argon2_conf = Argon2Conf {
            threads: 4,
            mem_cost: 2 * 1024 * 1024 * 1024,
            time_cost: 2,
            variant: argon2::Variant::Argon2id.as_u32(),
            version: argon2::Version::Version13.as_u32(),
        };

        return BackupManagerConf {
            chunk_root_dir,
            db_path,
            manifest_path,
            chunker_conf,
            argon2_conf,
        };
    }
}

#[derive(Debug)]
pub struct BackupManager {
    inode_db: Mutex<InodeDb>,
    chunk_db: Mutex<ChunkDb>,
    manifest: Manifest,
    keys: CryptoKeys,
    sig_key: EncKey,
    database: sled::Db,
}

impl BackupManager {
    pub fn initialize_backup_manager(
        manifest_path: &Path,
        password: &str,
    ) -> Result<BackupManager> {
        let manifest = fs::read_to_string(manifest_path)?;

        let manifest: SignedManifest = serde_json::from_str(&manifest)?;

        let mut crypto_root = argon2::hash_raw(
            password.as_bytes(),
            &manifest.get_salt(),
            &manifest.get_argon2config()?,
        )?;

        let sig_key: Vec<u8> = crypto_root.drain(..KEY_SIZE).collect();
        let sig_key = EncKey::try_from(sig_key.as_slice())?;

        let manifest = manifest.verify(&sig_key)?;

        // Only now we are sure that no tapering occured in manifest!

        let key_encryption_keys: Vec<u8> = crypto_root.drain(..CRYPTO_KEYS_SIZE).collect();
        let key_encryption_keys =
            <[u8; CRYPTO_KEYS_SIZE]>::try_from(key_encryption_keys.as_slice())?;
        let key_encryption_keys = KeyEncryptionKeys::from(key_encryption_keys);

        let keys = manifest.keys.decrypt(key_encryption_keys);

        // read database
        let db: sled::Db = sled::open(manifest.db_path.clone())?;
        if !db.was_recovered() {
            return Err(BackrubError::SledDbDidNotExist(manifest.db_path).into());
        }

        let inode_tree = db.open_tree(b"inodes")?;
        let chunk_tree = db.open_tree(b"chunks")?;

        let inode_db = InodeDb::new(inode_tree, keys.inode_enc_key, keys.inode_hash_key)?;

        let chunk_db = ChunkDb::restore(
            chunk_tree,
            keys.chunk_enc_key,
            manifest.chunk_db_state.clone(),
        )?;

        let manager = BackupManager {
            inode_db: Mutex::new(inode_db),
            chunk_db: Mutex::new(chunk_db),
            manifest,
            keys,
            sig_key,
            database: db,
        };

        Ok(manager)
    }

    pub fn new(config: BackupManagerConf, password: &str) -> Result<BackupManager> {
        let mut salt = [0u8; SALT_SIZE];
        OsRng.fill_bytes(&mut salt);

        let mut crypto_root = argon2::hash_raw(
            password.as_bytes(),
            &salt,
            &config.argon2_conf.as_argon2config()?,
        )?;

        // Derive signature key
        let sig_key: Vec<u8> = crypto_root.drain(..KEY_SIZE).collect();
        let sig_key = EncKey::try_from(sig_key.as_slice())?;

        // Derive keys
        let key_encryption_keys: Vec<u8> = crypto_root.drain(..CRYPTO_KEYS_SIZE).collect();
        let key_encryption_keys =
            <[u8; CRYPTO_KEYS_SIZE]>::try_from(key_encryption_keys.as_slice())?;
        let key_encryption_keys = KeyEncryptionKeys::from(key_encryption_keys);

        let keys = CryptoKeys::new();

        let enc_keys = keys.encrypt(key_encryption_keys);

        // create database
        let db: sled::Db = sled::open(config.clone().db_path)?;
        if db.was_recovered() {
            return Err(BackrubError::SledDbAlreadyExists(config.db_path).into());
        }

        // setup inode and chunk databases
        let inode_tree = db.open_tree(b"inodes")?;
        let chunk_tree = db.open_tree(b"chunks")?;

        let inode_db = InodeDb::new(inode_tree, keys.inode_enc_key, keys.inode_hash_key)?;
        let chunk_db = ChunkDb::new(chunk_tree, keys.chunk_enc_key)?;

        // create Manifest
        let manifest = Manifest {
            salt: salt,
            chunk_root_dir: config.chunk_root_dir,
            db_path: config.db_path,
            version: env!("CARGO_PKG_VERSION").to_string(),
            chunker_conf: config.chunker_conf,
            keys: enc_keys,
            argon2_conf: config.argon2_conf,
            chunk_db_state: chunk_db.state.clone(),
        };

        // create BackupManager
        let manager = BackupManager {
            inode_db: Mutex::new(inode_db),
            chunk_db: Mutex::new(chunk_db),
            manifest,
            keys,
            sig_key,
            database: db,
        };

        // write Manifest
        manager.write_manifet(config.manifest_path.as_path())?;

        Ok(manager)
    }

    fn write_manifet(&self, manifest_path: &Path) -> Result<()> {
        // Lock mutex
        let chunk_db = block_on(self.chunk_db.lock());

        // copy manifest
        let mut manifest = self.manifest.clone();

        // update manifest
        manifest.chunk_db_state = chunk_db.state.clone();

        // sign manifest
        let signed = manifest.sign(&self.sig_key)?;

        // serialize
        let manifest_json = serde_json::to_string(&signed)?;

        // write manifest
        let mut file = fs::File::create(manifest_path)?;
        file.write_all(manifest_json.as_bytes())?;

        Ok(())
    }

    /// Cerates a new backup
    pub fn create_backup(&mut self, name: &str, path: &Path, conf: &BackupConf) -> Result<()> {
        if !path.is_dir() {
            return Err(BackrubError::BackupRootMustBeDir(path.to_path_buf()).into());
        }

        self.backup_dir(path, conf);

        todo!("create backup db entry")
    }

    /// performs all backup operations for a directory
    fn backup_dir(&mut self, path: &Path, conf: &BackupConf) {}
}

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
    minimum_chunk_size: u64,
    average_chunk_size: u64,
    maximum_chunk_size: u64,
}

#[derive(Clone, Hash, Copy, Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EncCryptoKeys {
    enc_chunk_hash_key: EncKey,
    enc_chunk_enc_key: EncKey,
    enc_inode_hash_key: EncKey,
    enc_inode_enc_key: EncKey,
}

impl EncCryptoKeys {
    fn decrypt(&self, keys: KeyEncryptionKeys) -> CryptoKeys {
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
    key_chunk_hash_key: EncKey,
    key_chunk_enc_key: EncKey,
    key_inode_hash_key: EncKey,
    key_inode_enc_key: EncKey,
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
struct CryptoKeys {
    chunk_hash_key: EncKey,
    chunk_enc_key: EncKey,
    inode_hash_key: EncKey,
    inode_enc_key: EncKey,
}

impl CryptoKeys {
    fn new() -> Self {
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
    fn encrypt(&self, keys: KeyEncryptionKeys) -> EncCryptoKeys {
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
    salt: [u8; SALT_SIZE],
    chunk_root_dir: PathBuf,
    db_path: PathBuf,
    version: String,
    chunker_conf: ChunkerConf,
    keys: EncCryptoKeys,
    //completed_backups: BTreeMap<BackupHash,Vec<u8>>,
    argon2_conf: Argon2Conf,
    chunk_db_state: ChunkDbState,
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

#[derive(Clone, Hash, Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CryptoCtx {
    nonce: EncNonce,
    data: Vec<u8>,
}

use std::{error, fmt};

/// Error type for errors that are specific for backrub.
///
/// For all practical purposes this will be wrapped into [Error].
#[derive(Clone, Hash, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum BackrubError {
    SledKeyLengthError,
    SledTreeNotEmpty,
    SledDbAlreadyExists(PathBuf),
    SledDbDidNotExist(PathBuf),
    SelfTestError,
    InvalidSignature,
    BackupRootMustBeDir(PathBuf),
}

impl fmt::Display for BackrubError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            BackrubError::InvalidSignature => {
                write!(
                    f,
                    "InvalidSignature: a signature is invalid, this could be a sign of tampering"
                )
            }
            BackrubError::BackupRootMustBeDir(path) => {
                write!(
                    f,
                    "BackupRootMustBeDir: the root of any backup must be a directory, got \"{}\"",
                    path.display()
                )
            }
            BackrubError::SledDbAlreadyExists(path) => {
                write!(
                    f,
                    "SledDbAlreadyExists: a sled database is already existing at given path \"{}\"",
                    path.display()
                )
            }
            BackrubError::SledDbDidNotExist(path) => {
                write!(
                    f,
                    "SledDbDidNotExist: a sled database was NOT existing at given path \"{}\"",
                    path.display()
                )
            }
            BackrubError::SledTreeNotEmpty => {
                write!(
                    f,
                    "SledTreeNotEmpty: a sled tree has a length bigger than 0"
                )
            }
            BackrubError::SledKeyLengthError => {
                write!(
                    f,
                    "SledKeyLengthError: a sled key seems to be of wrong length"
                )
            }
            BackrubError::SelfTestError => {
                write!(f, "SelfTestError: a sled key - value pair is corrupted")
            }
        }
    }
}

impl error::Error for BackrubError {}

macro_rules! impl_error_enum{
    (
        $(#[$meta:meta])*
        $vis:vis enum $enum_name:ident {
            $(
            $(#[$field_meta:meta])*
            $field_type:ident ( $enc_type:ty )
            ),*$(,)+
        }
    ) => {
        $(#[$meta])*
        $vis enum $enum_name{
            $(
            $(#[$field_meta:meta])*
            $field_type ( $enc_type ),
            )*
        }

        impl fmt::Display for $enum_name {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                match self {
                    $($enum_name::$field_type ( error ) => {
                        write!(f, "{}::{}: ", stringify!($enum_name), stringify!($field_type))?;
                        error.fmt(f)
                    })*
                }
            }
        }

        $(
        impl From<$enc_type> for $enum_name {
           fn from(err: $enc_type) -> Self {
               $enum_name::$field_type(err)
           }
        }
        )*

        impl std::error::Error for $enum_name {}
    }
}

impl_error_enum!(
    /// Encapsulating error type for all possible kinds of errors in backrub.
    ///
    /// This is [Error] is used by [Result].
    #[derive(Debug)]
    pub enum Error {
        CryptoError(chacha20poly1305::aead::Error),
        BackrubError(BackrubError),
        SledError(sled::Error),
        BincodeError(Box<bincode::ErrorKind>),
        IoError(std::io::Error),
        TryFromSliceError(std::array::TryFromSliceError),
        SerdeJsonError(serde_json::Error),
        Argon2Error(argon2::Error),
    }
);

/// Backrub specific result wrapper, using [Error].
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Hash, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChunkDbState {
    unused_paths: Vec<PathBuf>,
    path_gen: FilePathGen,
}

#[derive(Clone, Hash, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct ChunkDbEntry {
    ref_count: RefCount,
    file_name: PathBuf,
}

impl Encrypt for ChunkDbEntry {}

/// ChunkDb manages mappings from chunk hashes to file names
///
/// The backuped chunks are supposed to be encrypted and stored under the filenames provided by this
#[derive(Debug)]
pub struct ChunkDb {
    chunk_map: sled::Tree,
    state: ChunkDbState,
    chunk_enc_key: EncKey,
}

impl ChunkDb {
    /// Check the ChunkDb contents for errors
    ///
    /// This is a O(n) operation
    pub fn self_test(&self) -> Result<()> {
        for data in self.chunk_map.iter() {
            let (key, encrypted_data) = data?;
            // Check key
            if key.len() != HASH_SIZE {
                return Err(BackrubError::SledKeyLengthError.into());
            }
            let _: ChunkHash = key
                .chunks_exact(HASH_SIZE)
                .next()
                .map_or_else(
                    || Err::<&[u8], BackrubError>(BackrubError::SledKeyLengthError),
                    Ok,
                )?
                .try_into()?;
            // Check data
            let _ = ChunkDbEntry::decrypt(&encrypted_data, &self.chunk_enc_key)?;
        }
        Ok(())
    }

    /// Restores a saved ChunkDb
    ///
    /// Returns an [`Error`] when the [`Self::self_test()`] fails
    pub fn restore(
        tree: sled::Tree,
        chunk_enc_key: EncKey,
        state: ChunkDbState,
    ) -> Result<ChunkDb> {
        let cs = ChunkDb {
            state: state,
            chunk_map: tree,
            chunk_enc_key: chunk_enc_key,
        };
        cs.self_test()?;
        Ok(cs)
    }

    /// Creates a new **empty** ChunkDb
    ///
    /// Returns [`BackrubError::SledTreeNotEmpty`] if provided tree is not empty
    pub fn new(tree: sled::Tree, chunk_enc_key: EncKey) -> Result<ChunkDb> {
        if tree.len() != 0 {
            return Err(BackrubError::SledTreeNotEmpty.into());
        }
        let cs = ChunkDb {
            state: ChunkDbState {
                path_gen: FilePathGen::default(),
                unused_paths: Vec::<PathBuf>::default(),
            },
            chunk_map: tree,
            chunk_enc_key: chunk_enc_key,
        };
        Ok(cs)
    }

    /// Inserts a new [`ChunkHash`] into the database and returns a tuple [`(RefCount, PathBuf)`] of the reference count and the file name the chunk should be stored in
    pub fn insert(&mut self, key: &ChunkHash) -> Result<(RefCount, PathBuf)> {
        match self.chunk_map.remove(key)? {
            None => {
                let file_name = self.state.unused_paths
                                         .pop()
                                         .unwrap_or_else(||{
                                             PathBuf::from(self.state.path_gen.next()
                                                           .expect("BUG: Please contact me if you need more than 10^19 chunks, I'd really like to know the system you are on"))
                    });

                self.chunk_map.insert(
                    key,
                    ChunkDbEntry {
                        file_name: file_name.clone(),
                        ref_count: 1,
                    }
                    .encrypt(&self.chunk_enc_key)?,
                )?;

                Ok((1, file_name))
            }
            Some(old) => {
                let old = ChunkDbEntry::decrypt(&old, &self.chunk_enc_key)?;

                let ref_count = old.ref_count + 1;

                self.chunk_map.insert(
                    key,
                    ChunkDbEntry {
                        file_name: old.file_name.clone(),
                        ref_count,
                    }
                    .encrypt(&self.chunk_enc_key)?,
                )?;

                Ok((ref_count, old.file_name))
            }
        }
    }

    /// Removes a chunk reference and returns the reference count as well as the file name the chunk is supposed to be stored in.
    ///
    /// - Returns `Ok(None)` if chunk was not referenced (no file name is associated with that chunk hash)
    /// - Returns `Ok((0, <file_name>))` if the last reference to this chunk was removed indicating that the chunk file should be removed
    pub fn remove(&mut self, key: &ChunkHash) -> Result<Option<(RefCount, PathBuf)>> {
        match self.chunk_map.remove(key)? {
            None => Ok(None),
            Some(old) => {
                let old = ChunkDbEntry::decrypt(&old, &self.chunk_enc_key)?;
                if old.ref_count <= 1 {
                    // save old file name for reuse
                    self.state.unused_paths.push(old.file_name.clone());
                    Ok(Some((0, old.file_name)))
                } else {
                    let ref_count = old.ref_count - 1;
                    self.chunk_map.insert(
                        key,
                        ChunkDbEntry {
                            file_name: old.file_name.clone(),
                            ref_count,
                        }
                        .encrypt(&self.chunk_enc_key)?,
                    )?;
                    Ok(Some((ref_count, old.file_name)))
                }
            }
        }
    }

    /// Returns the number of stored chunks
    ///
    /// This performs a full O(n) scan
    pub fn len(&self) -> usize {
        self.chunk_map.len()
    }

    /// Returns a BTreeMap containing the contens of the internal mappings.
    ///
    /// This decrypts all contens creates a compleatly new map in memory
    pub fn get_mappings(&self) -> Result<BTreeMap<ChunkHash, (RefCount, PathBuf)>> {
        let mut result = BTreeMap::<ChunkHash, (RefCount, PathBuf)>::new();
        for data in self.chunk_map.iter() {
            let (key, encrypted_data) = data?;
            if key.len() != HASH_SIZE {
                return Err(BackrubError::SledKeyLengthError.into());
            } else {
                let key: ChunkHash = key
                    .chunks_exact(HASH_SIZE)
                    .next()
                    .map_or_else(
                        || Err::<&[u8], BackrubError>(BackrubError::SledKeyLengthError),
                        Ok,
                    )?
                    .try_into()?;
                let chunk_file = ChunkDbEntry::decrypt(&encrypted_data, &self.chunk_enc_key)?;
                result.insert(key, (chunk_file.ref_count, chunk_file.file_name));
            }
        }
        Ok(result)
    }

    pub fn get_entry(&self, key: &ChunkHash) -> Result<Option<(RefCount, PathBuf)>> {
        match self.chunk_map.get(key)? {
            None => Ok(None),
            Some(encrypted_data) => {
                let chunk_file = ChunkDbEntry::decrypt(&encrypted_data, &self.chunk_enc_key)?;
                Ok(Some((chunk_file.ref_count, chunk_file.file_name)))
            }
        }
    }

    pub fn get_ref_count(&self, key: &ChunkHash) -> Result<Option<RefCount>> {
        match self.get_entry(key)? {
            None => Ok(None),
            Some((ref_count, _file_name)) => Ok(Some(ref_count)),
        }
    }

    pub fn get_file_name(&self, key: &ChunkHash) -> Result<Option<PathBuf>> {
        match self.get_entry(key)? {
            None => Ok(None),
            Some((_ref_count, file_name)) => Ok(Some(file_name)),
        }
    }
}

#[derive(Clone, Hash, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct RcDbEntry<T> {
    data: T,
    ref_count: RefCount,
}

impl<T: Serialize + for<'a> Deserialize<'a>> Encrypt for RcDbEntry<T> {}

impl<T: Hashable> Hashable for RcDbEntry<T> {
    fn hash(&self) -> Result<blake3::Hash> {
        self.data.hash()
    }
    fn keyed_hash(&self, key: &EncKey) -> Result<blake3::Hash> {
        self.data.keyed_hash(key)
    }
}

/// Generic encrypted reference countig database with [sled] backend
#[derive(Debug)]
pub struct RcDb<T: Hashable + Serialize + for<'a> Deserialize<'a>> {
    tree: sled::Tree,
    data_enc_key: EncKey,
    data_hash_key: EncKey,
    entry_type: PhantomData<T>,
}

impl<T: Clone + Hashable + Serialize + for<'a> Deserialize<'a>> RcDb<T> {
    /// Database self test
    ///
    /// This performs a complete table scan checking all keys and integrity of all data
    pub fn self_test(&self) -> Result<()> {
        for data in self.tree.iter() {
            let (key, encrypted_data) = data?;

            // Check Key
            if key.len() != HASH_SIZE {
                return Err(BackrubError::SledKeyLengthError.into());
            }
            let key: Hash = key
                .chunks_exact(HASH_SIZE)
                .next()
                .map_or_else(
                    || Err::<&[u8], BackrubError>(BackrubError::SledKeyLengthError),
                    Ok,
                )?
                .try_into()?;

            // Check data
            let entry = RcDbEntry::<T>::decrypt(&encrypted_data, &self.data_enc_key)?;
            if key != Hash::from(*entry.data.keyed_hash(&self.data_hash_key)?.as_bytes()) {
                return Err(BackrubError::SelfTestError.into());
            }
        }
        Ok(())
    }

    /// Creates a new reference counting database from a `sled::Tree` and running a self test
    pub fn new(tree: sled::Tree, data_enc_key: EncKey, data_hash_key: EncKey) -> Result<RcDb<T>> {
        let db = RcDb {
            tree,
            data_enc_key,
            data_hash_key,
            entry_type: PhantomData,
        };
        db.self_test()?;
        Ok(db)
    }

    /// Returns the number of stored objects in the database
    ///
    /// **This performs an O(n) scan**
    pub fn len(&self) -> usize {
        self.tree.len()
    }

    /// Inserts data into the database
    /// If the same data is already stored the reference count is incremented
    pub fn insert(&mut self, data: T) -> Result<(RefCount, Hash)> {
        let key = Hash::from(*data.keyed_hash(&self.data_hash_key)?.as_bytes());
        match self.tree.remove(key)? {
            Some(old) => {
                let old = RcDbEntry::<T>::decrypt(&old, &self.data_enc_key)?;
                let ref_count = old.ref_count + 1;

                self.tree.insert(
                    key,
                    RcDbEntry { data, ref_count }.encrypt(&self.data_enc_key)?,
                )?;

                Ok((ref_count, key))
            }
            None => {
                let encrypted_entry =
                    RcDbEntry { data, ref_count: 1 }.encrypt(&self.data_enc_key)?;
                self.tree.insert(key, encrypted_entry)?;
                Ok((1, key))
            }
        }
    }

    /// Removes an instace of the referenced data from the database.
    /// If the reference count reaches 0 the element will be deleted.
    pub fn remove(&mut self, key: &Hash) -> Result<Option<(RefCount, T)>> {
        match self.tree.remove(key)? {
            None => Ok(None),
            Some(old) => {
                let old = RcDbEntry::decrypt(&old, &self.data_enc_key)?;
                if old.ref_count <= 1 {
                    Ok(Some((0, old.data)))
                } else {
                    let ref_count = old.ref_count - 1;

                    let encrypted_entry = RcDbEntry {
                        data: old.data.clone(),
                        ref_count,
                    }
                    .encrypt(&self.data_enc_key)?;
                    self.tree.insert(key, encrypted_entry)?;
                    Ok(Some((ref_count, old.data)))
                }
            }
        }
    }

    /// Deletes the referenced entry from the database regardless of the reference count
    pub fn purge(&mut self, key: &Hash) -> Result<Option<(RefCount, T)>> {
        match self.tree.remove(key)? {
            None => Ok(None),
            Some(old) => {
                let old = RcDbEntry::decrypt(&old, &self.data_enc_key)?;
                Ok(Some((old.ref_count, old.data)))
            }
        }
    }

    /// Gets the current refercence count and data
    pub fn get_data_db_entry(&self, key: &Hash) -> Result<Option<(RefCount, T)>> {
        match self.tree.get(key)? {
            None => Ok(None),
            Some(encrypted_data) => {
                let entry = RcDbEntry::decrypt(&encrypted_data, &self.data_enc_key)?;
                Ok(Some((entry.ref_count, entry.data)))
            }
        }
    }

    /// Gets only the referenced data
    pub fn get_data(&self, key: &Hash) -> Result<Option<T>> {
        match self.get_data_db_entry(key)? {
            None => Ok(None),
            Some((_ref_count, data)) => Ok(Some(data)),
        }
    }

    /// Gets only the reference count
    pub fn get_ref_count(&self, key: &Hash) -> Result<Option<RefCount>> {
        match self.get_data_db_entry(key)? {
            None => Ok(None),
            Some((ref_count, _data)) => Ok(Some(ref_count)),
        }
    }

    /// Returns a complete in memory representation of the database
    pub fn get_mappings(&self) -> Result<BTreeMap<Hash, (RefCount, T)>> {
        let mut result = BTreeMap::<Hash, (RefCount, T)>::new();
        for data in self.tree.iter() {
            let (key, encrypted_data) = data?;
            if key.len() != HASH_SIZE {
                return Err(BackrubError::SledKeyLengthError.into());
            } else {
                let hash: Hash = key
                    .chunks_exact(HASH_SIZE)
                    .next()
                    .map_or_else(
                        || Err::<&[u8], BackrubError>(BackrubError::SledKeyLengthError),
                        Ok,
                    )?
                    .try_into()?;
                let entry = RcDbEntry::decrypt(&encrypted_data, &self.data_enc_key)?;
                result.insert(hash, (entry.ref_count, entry.data));
            }
        }
        Ok(result)
    }
}

#[derive(Clone, Hash, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct InodeDbEntry {
    inode: Inode,
    ref_count: RefCount,
}

impl Encrypt for InodeDbEntry {}

#[derive(Debug)]
pub struct InodeDb {
    tree: sled::Tree,
    inode_enc_key: EncKey,
    inode_hash_key: EncKey,
}

impl InodeDb {
    pub fn self_test(&self) -> Result<()> {
        for data in self.tree.iter() {
            let (key, encrypted_data) = data?;

            // Check Key
            if key.len() != HASH_SIZE {
                return Err(BackrubError::SledKeyLengthError.into());
            }
            let key: InodeHash = key
                .chunks_exact(HASH_SIZE)
                .next()
                .map_or_else(
                    || Err::<&[u8], BackrubError>(BackrubError::SledKeyLengthError),
                    Ok,
                )?
                .try_into()?;

            // Check data
            let entry = InodeDbEntry::decrypt(&encrypted_data, &self.inode_enc_key)?;
            if key != InodeHash::from(*entry.inode.keyed_hash(&self.inode_hash_key)?.as_bytes()) {
                return Err(BackrubError::SelfTestError.into());
            }
        }
        Ok(())
    }

    pub fn new(tree: sled::Tree, inode_enc_key: EncKey, inode_hash_key: EncKey) -> Result<InodeDb> {
        let db = InodeDb {
            tree,
            inode_enc_key,
            inode_hash_key,
        };
        db.self_test()?;
        Ok(db)
    }

    pub fn len(&self) -> usize {
        self.tree.len()
    }

    pub fn insert(&mut self, inode: Inode) -> Result<(RefCount, InodeHash)> {
        let key = InodeHash::from(*inode.keyed_hash(&self.inode_hash_key)?.as_bytes());
        match self.tree.remove(key)? {
            Some(old) => {
                let old = InodeDbEntry::decrypt(&old, &self.inode_enc_key)?;
                let ref_count = old.ref_count + 1;

                self.tree.insert(
                    key,
                    InodeDbEntry { inode, ref_count }.encrypt(&self.inode_enc_key)?,
                )?;

                Ok((ref_count, key))
            }
            None => {
                let encrypted_entry = InodeDbEntry {
                    inode,
                    ref_count: 1,
                }
                .encrypt(&self.inode_enc_key)?;
                self.tree.insert(key, encrypted_entry)?;
                Ok((1, key))
            }
        }
    }

    pub fn remove(&mut self, key: &InodeHash) -> Result<Option<(RefCount, Inode)>> {
        match self.tree.remove(key)? {
            None => Ok(None),
            Some(old) => {
                let old = InodeDbEntry::decrypt(&old, &self.inode_enc_key)?;
                if old.ref_count <= 1 {
                    Ok(Some((0, old.inode)))
                } else {
                    let ref_count = old.ref_count - 1;

                    let encrypted_entry = InodeDbEntry {
                        inode: old.inode.clone(),
                        ref_count,
                    }
                    .encrypt(&self.inode_enc_key)?;
                    self.tree.insert(key, encrypted_entry)?;
                    Ok(Some((ref_count, old.inode)))
                }
            }
        }
    }

    pub fn get_inode_db_entry(&self, key: &InodeHash) -> Result<Option<(RefCount, Inode)>> {
        match self.tree.get(key)? {
            None => Ok(None),
            Some(encrypted_data) => {
                let entry = InodeDbEntry::decrypt(&encrypted_data, &self.inode_enc_key)?;
                Ok(Some((entry.ref_count, entry.inode)))
            }
        }
    }

    pub fn get_inode(&self, key: &InodeHash) -> Result<Option<Inode>> {
        match self.get_inode_db_entry(key)? {
            None => Ok(None),
            Some((_ref_count, inode)) => Ok(Some(inode)),
        }
    }

    pub fn get_ref_count(&self, key: &InodeHash) -> Result<Option<RefCount>> {
        match self.get_inode_db_entry(key)? {
            None => Ok(None),
            Some((ref_count, _inode)) => Ok(Some(ref_count)),
        }
    }

    pub fn get_mappings(&self) -> Result<BTreeMap<InodeHash, (RefCount, Inode)>> {
        let mut result = BTreeMap::<InodeHash, (RefCount, Inode)>::new();
        for data in self.tree.iter() {
            let (key, encrypted_data) = data?;
            if key.len() != HASH_SIZE {
                return Err(BackrubError::SledKeyLengthError.into());
            } else {
                let hash: InodeHash = key
                    .chunks_exact(HASH_SIZE)
                    .next()
                    .map_or_else(
                        || Err::<&[u8], BackrubError>(BackrubError::SledKeyLengthError),
                        Ok,
                    )?
                    .try_into()?;
                let entry = InodeDbEntry::decrypt(&encrypted_data, &self.inode_enc_key)?;
                result.insert(hash, (entry.ref_count, entry.inode));
            }
        }
        Ok(result)
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
