pub mod structs {
    use async_std::sync::Mutex;
    use chacha20poly1305::{
        aead::{rand_core::RngCore, Aead, AeadCore, KeyInit, OsRng},
        XChaCha20Poly1305,
    };
    use flate2::write::{DeflateDecoder, DeflateEncoder};
    use flate2::Compression;
    use generic_array::GenericArray;
    use serde::{Deserialize, Serialize};
    use std::collections::BTreeMap;
    use std::io::prelude::*;
    use std::path::{Path, PathBuf};
    use typenum::{
        bit::{B0, B1},
        uint::{UInt, UTerm},
    };

    const HASH_SIZE: usize = 32;
    const KEY_SIZE: usize = 32;
    const NONCE_SIZE: usize = 24;

    type RefCount = usize;

    pub trait Hashable: Serialize {
        fn hash(&self) -> Result<blake3::Hash> {
            let serialized = bincode::serialize(self)?;
            Ok(blake3::hash(&serialized))
        }

        fn keyed_hash(&self, key: &EncKey) -> Result<blake3::Hash> {
            let serialized = bincode::serialize(self)?;
            Ok(blake3::keyed_hash(&key.as_array(), &serialized))
        }
    }

    pub trait Encrypt: Serialize + for<'a> Deserialize<'a> {
        /// Generic function to encrypt data in backrub
        fn encrypt(&self, key: &EncKey) -> Result<Vec<u8>> {
            // generate nonce
            let nonce: EncNonce = XChaCha20Poly1305::generate_nonce(&mut OsRng).into();
            // setup the cipher
            let cipher = XChaCha20Poly1305::new(&key.as_array().into());
            // convert data to Vec<u8>
            let serialized_data = bincode::serialize(self)?;
            // encrypt the data
            let encrypted_data = cipher.encrypt(&nonce.as_array().into(), &serialized_data[..])?;
            // construct CryptoCtx using the nonce and the encrypted data
            let ctx = CryptoCtx {
                nonce,
                data: encrypted_data,
            };
            // convert CryptoCtx to Vec<u8>
            Ok(bincode::serialize(&ctx)?)
        }

        /// Generic function to decrypt data encrypted by backrub
        fn decrypt(data: &[u8], key: &EncKey) -> Result<Self> {
            // decode encrypted data to split nonce and encrypted data
            let ctx = bincode::deserialize::<CryptoCtx>(data)?;
            // setup the cipher
            let cipher = XChaCha20Poly1305::new(&key.as_array().into());
            // decrypt the data
            let decrypted_data = cipher.decrypt(&ctx.nonce.as_array().into(), &ctx.data[..])?;
            // convert decrypted data to the target data type
            Ok(bincode::deserialize(&decrypted_data)?)
        }

        /// Generic function to compress and encrypt data in backrub
        fn compress_and_encrypt(&self, key: &EncKey) -> Result<Vec<u8>> {
            // generate nonce
            let nonce: EncNonce = XChaCha20Poly1305::generate_nonce(&mut OsRng).into();
            // setup the cipher
            let cipher = XChaCha20Poly1305::new(&key.as_array().into());
            // convert data to Vec<u8>
            let serialized_data = bincode::serialize(self)?;

            // compress the data
            let mut compressor = DeflateEncoder::new(Vec::new(), Compression::default());
            compressor.write_all(&serialized_data[..])?;
            let compressed_data = compressor.finish()?;
            // encrypt the data
            let encrypted_data = cipher.encrypt(&nonce.as_array().into(), &compressed_data[..])?;
            // construct CryptoCtx using the nonce and the encrypted data
            let ctx = CryptoCtx {
                nonce,
                data: encrypted_data,
            };
            // convert CryptoCtx to Vec<u8>
            Ok(bincode::serialize(&ctx)?)
        }

        /// Generic function to decrypt and uncompress data encrypted by backrub
        fn decrypt_and_uncompress(data: &[u8], key: &EncKey) -> Result<Self> {
            // decode encrypted data to split nonce and encrypted data
            let ctx = bincode::deserialize::<CryptoCtx>(data)?;
            // setup the cipher
            let cipher = XChaCha20Poly1305::new(&key.as_array().into());
            // decrypt the data
            let decrypted_data = cipher.decrypt(&ctx.nonce.as_array().into(), &ctx.data[..])?;
            // decompress decrypted data
            let mut uncompressed_data = Vec::new();
            let mut deflater = DeflateDecoder::new(uncompressed_data);
            deflater.write_all(&decrypted_data)?;
            uncompressed_data = deflater.finish()?;
            // deserialize uncompressed data
            Ok(bincode::deserialize(&uncompressed_data)?)
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
            array: GenericArray<
                u8,
                UInt<UInt<UInt<UInt<UInt<UInt<UTerm, B1>, B0>, B0>, B0>, B0>, B0>,
            >,
        ) -> Self {
            EncKey(<[u8; KEY_SIZE]>::from(array))
        }
    }

    impl EncKey {
        pub fn as_array(&self) -> [u8; KEY_SIZE] {
            self.0
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
        pub fn as_array(&self) -> [u8; NONCE_SIZE] {
            self.0
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
            array: GenericArray<
                u8,
                UInt<UInt<UInt<UInt<UInt<UInt<UTerm, B1>, B0>, B0>, B0>, B0>, B0>,
            >,
        ) -> Self {
            SigKey(<[u8; KEY_SIZE]>::from(array))
        }
    }

    impl SigKey {
        pub fn as_array(&self) -> [u8; KEY_SIZE] {
            self.0
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

    #[derive(Debug)]
    pub struct BackupManager {
        inode_db: Mutex<InodeDb>,
        chunk_db: Mutex<ChunkDb>,
        manifest: Manifest,
    }

    impl BackupManager {
        pub fn initialize_backup_manager(
            manifest_path: &Path,
            password: &str,
        ) -> Result<BackupManager> {
            let manager: BackupManager = todo!();

            Ok(manager)
        }

        fn create_backup(name: &str, path: &Path, conf: &BackupConf) -> Result<()> {
            todo!()
        }

        async fn bla(&mut self) -> () {
            todo!()
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
    #[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
    pub struct Metadata {
        mode: u32,
        uid: u32,
        gid: u32,
        mtime: i64,
        mtime_ns: i64,
        ctime: i64,
        ctime_ns: i64,
    }

    #[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
    pub struct ChunkerConf {
        minimum_chunk_size: usize,
        average_chunk_size: usize,
        maximum_chunk_size: usize,
    }

    #[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
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

    #[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
    pub struct KeyEncryptionKeys {
        key_chunk_hash_key: EncKey,
        key_chunk_enc_key: EncKey,
        key_inode_hash_key: EncKey,
        key_inode_enc_key: EncKey,
    }

    impl From<[u8; KEY_SIZE * 4]> for KeyEncryptionKeys {
        fn from(keys: [u8; KEY_SIZE * 4]) -> Self {
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

    #[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
    struct CryptoKeys {
        chunk_hash_key: EncKey,
        chunk_enc_key: EncKey,
        inode_hash_key: EncKey,
        inode_enc_key: EncKey,
    }

    impl CryptoKeys {
        fn new() -> Self {
            let mut keys = [0u8; KEY_SIZE * 4];
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

    #[derive(Clone, Default, Debug, Serialize, Deserialize)]
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
    }

    #[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
    pub struct Manifest {
        salt: [u8; 32],
        chunk_root_dir: PathBuf,
        db_path: PathBuf,
        version: String,
        chunker_conf: ChunkerConf,
        keys: EncCryptoKeys,
        compleated_backups: usize,
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

    #[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
    pub struct Backup {
        timestamp: String,
        name: String,
        root: InodeHash,
    }

    #[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
    pub struct Symlink {
        relpath: PathBuf,
        target: PathBuf,
        metadata: Metadata,
    }

    #[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
    pub struct Directory {
        relpath: PathBuf,
        metadata: Metadata,
        contents: Vec<InodeHash>,
    }

    #[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
    pub struct File {
        relpath: PathBuf,
        chunk_ids: Vec<ChunkHash>,
        metadata: Metadata,
    }

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
    pub enum Inode {
        File(File),
        Directory(Directory),
        Symlink(Symlink),
    }

    impl Hashable for Inode {}

    #[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
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

    #[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
    struct FilePathGen(u64);
    impl FilePathGen {
        pub fn new() -> FilePathGen {
            FilePathGen::default()
        }
        pub fn from_U64(n: u64) -> FilePathGen {
            FilePathGen { 0: n }
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

    #[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
    pub struct CryptoCtx {
        nonce: EncNonce,
        data: Vec<u8>,
    }

    use std::{error, fmt};

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
    pub enum BackrubError {
        SledKeyLengthError,
        SledTreeNotEmpty,
        SelfTestError,
        InvalidSignature,
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

    #[derive(Debug)]
    pub enum Error {
        CryptoError(chacha20poly1305::aead::Error),
        BackrubError(BackrubError),
        SledError(sled::Error),
        BincodeError(bincode::ErrorKind),
        IoError(std::io::Error),
        TryFromSliceError(std::array::TryFromSliceError),
        OnceCellError(String),
    }

    impl fmt::Display for Error {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            match self {
                Error::CryptoError(error) => {
                    write!(f, "backrub::Error::Crypto: ");
                    error.fmt(f)
                }
                Error::BackrubError(error) => {
                    write!(f, "backrub::Error::BackrubError: ");
                    error.fmt(f)
                }
                Error::SledError(error) => {
                    write!(f, "backrub::Error::SledError: ");
                    error.fmt(f)
                }
                Error::BincodeError(error) => {
                    write!(f, "backrub::Error::BincodeError: ");
                    error.fmt(f)
                }
                Error::IoError(error) => {
                    write!(f, "backrub::Error::IoError: ");
                    error.fmt(f)
                }
                Error::TryFromSliceError(error) => {
                    write!(f, "backrub::Error::TryFromSliceError: ");
                    error.fmt(f)
                }
                Error::OnceCellError(msg) => {
                    write!(f, "backrub::Error::OnceCellError: {}", msg)
                }
            }
        }
    }

    impl error::Error for Error {}

    impl From<std::array::TryFromSliceError> for Error {
        fn from(err: std::array::TryFromSliceError) -> Self {
            Error::TryFromSliceError(err)
        }
    }

    impl From<String> for Error {
        fn from(err: String) -> Self {
            Error::OnceCellError(err)
        }
    }
    impl From<Box<bincode::ErrorKind>> for Error {
        fn from(err_ptr: Box<bincode::ErrorKind>) -> Self {
            Error::BincodeError(*err_ptr)
        }
    }

    impl From<chacha20poly1305::aead::Error> for Error {
        fn from(err: chacha20poly1305::aead::Error) -> Self {
            Error::CryptoError(err)
        }
    }

    impl From<sled::Error> for Error {
        fn from(err: sled::Error) -> Self {
            Error::SledError(err)
        }
    }

    impl From<BackrubError> for Error {
        fn from(err: BackrubError) -> Self {
            Error::BackrubError(err)
        }
    }

    impl From<std::io::Error> for Error {
        fn from(err: std::io::Error) -> Self {
            Error::IoError(err)
        }
    }

    type Result<T> = std::result::Result<T, Error>;

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
    pub struct ChunkDbState {
        unused_paths: Vec<PathBuf>,
        path_gen: FilePathGen,
    }

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
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

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
    struct InodeDbEntry {
        inode: Inode,
        ref_count: RefCount,
    }

    impl Encrypt for InodeDbEntry {}

    #[derive(Debug)]
    struct InodeDb {
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
                if key != InodeHash::from(*entry.inode.keyed_hash(&self.inode_hash_key)?.as_bytes())
                {
                    return Err(BackrubError::SelfTestError.into());
                }
            }
            Ok(())
        }

        pub fn new(
            tree: sled::Tree,
            inode_enc_key: EncKey,
            inode_hash_key: EncKey,
        ) -> Result<InodeDb> {
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

            let mut raw_keys = [0u8; KEY_SIZE * 4];
            OsRng.fill_bytes(&mut raw_keys);
            let kek = KeyEncryptionKeys::from(raw_keys);

            let enc_keys = ck.encrypt(kek.clone());
            let dec_keys = enc_keys.decrypt(kek);

            assert_eq!(ck, dec_keys);
        }
    }
}
