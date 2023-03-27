pub mod structs {
    use flate2::write::{DeflateDecoder, DeflateEncoder};
    use flate2::Compression;
    use once_cell::sync::OnceCell;
    use serde::{Deserialize, Serialize};
    use std::collections::{BTreeMap, HashMap};
    use std::io::prelude::*;
    use std::path::{PathBuf, Path};
    use async_std::sync::Mutex;

    const HASH_SIZE: usize = 32;
    const KEY_SIZE: usize = 32;
    const NONCE_SIZE: usize = 24;

    type RefCount = usize;

    type ChunkHash = [u8; HASH_SIZE];
    type InodeHash = [u8; HASH_SIZE];
    //type BackupHash = [u8; HASH_SIZE];
    type EncNonce = [u8; NONCE_SIZE];
    type EncKey = [u8; KEY_SIZE];
    type SigKey = [u8; KEY_SIZE];


    #[derive(Debug)]
    pub struct BackupManager {
        inode_db: Mutex<InodeDb>,
        chunk_db: Mutex<ChunkDb>,
        manifest: Manifest,
    }


    impl BackupManager {
        pub fn initialize_backup_manager(manifest_path: &Path, password: &str) -> Result<BackupManager> {

            let manager: BackupManager = todo!();


            Ok(manager)
        }

        fn create_backup(name: &str, path: &Path) -> Result<()> {
            todo!()
        }

        async fn bla(&mut self) -> () {
            todo!()
        }
    }

    #[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
    pub struct RuntimeConf {
        manifest_sig_key: SigKey,
        chunk_encryption_key: EncKey,
        chunk_hash_key: EncKey,
        inode_encryption_key: EncKey,
        inode_hash_key: EncKey,
        backup_encryption_key: EncKey,
    }


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
    pub struct Manifest {
        salt: [u8; 32],
        chunk_root_dir: PathBuf,
        backup_db_path: PathBuf,
        inode_db_path: PathBuf,
        version: String,
        chunker_conf: ChunkerConf,
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

    trait Hashable: Serialize {
        fn hash(&self) -> Result<blake3::Hash> {
            let serialized = bincode::serialize(self)?;
            Ok(blake3::hash(&serialized))
        }

        fn keyed_hash(&self, key: &EncKey) -> Result<blake3::Hash> {
            let serialized = bincode::serialize(self)?;
            Ok(blake3::keyed_hash(key, &serialized))
        }
    }

    #[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
    struct Chunk {
        data: Vec<u8>,
    }

    impl Hashable for Chunk {}

    pub fn log2u64(x: u64) -> Option<u64> {
        if x > 0 {
            Some(std::mem::size_of::<u64>() as u64 * 8u64 - x.leading_zeros() as u64 - 1u64)
        } else {
            None
        }
    }

    #[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
    pub struct FilePathGen(u64);
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
    pub enum DedupError {
        SledKeyLengthError,
        SelfTestError,
    }

    impl fmt::Display for DedupError {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            match self {
                DedupError::SledKeyLengthError => {
                    write!(
                        f,
                        "SledKeyLengthError: a sled key seems to be of wrong length"
                    )
                }
                DedupError::SelfTestError => {
                    write!(f, "SelfTestError: a sled key - value pair is corrupted")
                }
            }
        }
    }

    impl error::Error for DedupError {}

    #[derive(Debug)]
    pub enum Error {
        CryptoError(chacha20poly1305::aead::Error),
        DedupError(DedupError),
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
                    write!(f, "dedup::Error::Crypto: ");
                    error.fmt(f)
                }
                Error::DedupError(error) => {
                    write!(f, "dedup::Error::DedupError: ");
                    error.fmt(f)
                }
                Error::SledError(error) => {
                    write!(f, "dedup::Error::SledError: ");
                    error.fmt(f)
                }
                Error::BincodeError(error) => {
                    write!(f, "dedup::Error::BincodeError: ");
                    error.fmt(f)
                }
                Error::IoError(error) => {
                    write!(f, "dedup::Error::IoError: ");
                    error.fmt(f)
                }
                Error::TryFromSliceError(error) => {
                    write!(f, "dedup::Error::TryFromSliceError: ");
                    error.fmt(f)
                }
                Error::OnceCellError(msg) => {
                    write!(f, "dedup::Error::OnceCellError: {}", msg)
                }
            }
        }
    }

    impl error::Error for Error {}

    impl std::convert::From<std::array::TryFromSliceError> for Error {
        fn from(err: std::array::TryFromSliceError) -> Self {
            Error::TryFromSliceError(err)
        }
    }

    impl std::convert::From<String> for Error {
        fn from(err: String ) -> Self {
            Error::OnceCellError(err)
        }
    }
    impl std::convert::From<Box<bincode::ErrorKind>> for Error {
        fn from(err_ptr: Box<bincode::ErrorKind>) -> Self {
            Error::BincodeError(*err_ptr)
        }
    }

    impl std::convert::From<chacha20poly1305::aead::Error> for Error {
        fn from(err: chacha20poly1305::aead::Error) -> Self {
            Error::CryptoError(err)
        }
    }

    impl std::convert::From<sled::Error> for Error {
        fn from(err: sled::Error) -> Self {
            Error::SledError(err)
        }
    }

    impl std::convert::From<DedupError> for Error {
        fn from(err: DedupError) -> Self {
            Error::DedupError(err)
        }
    }

    impl std::convert::From<std::io::Error> for Error {
        fn from(err: std::io::Error) -> Self {
            Error::IoError(err)
        }
    }

    type Result<T> = std::result::Result<T, Error>;

    use chacha20poly1305::{
        aead::{Aead, AeadCore, KeyInit, OsRng},
        Nonce, XChaCha20Poly1305,
    };

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
    pub struct ChunkDbState {
        pub unused_paths: Vec<PathBuf>,
        pub path_gen: FilePathGen,
    }

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
    struct ChunkDbEntry {
        ref_count: RefCount,
        file_name: PathBuf,
    }

    #[derive(Debug)]
    struct ChunkDb {
        chunk_map: sled::Tree,
        state: ChunkDbState,
        chunk_enc_key: EncKey,
    }

    impl ChunkDb {
        pub fn self_test(&self) -> Result<()> {
            /// Check the ChunkDb contents for errors
            ///
            /// This is a O(n) operation
            for data in self.chunk_map.iter() {
                let (key, encrypted_data) = data?;
                // Check key
                if key.len() != HASH_SIZE {
                    return Err(DedupError::SledKeyLengthError.into());
                }
                let _: ChunkHash = key
                    .chunks_exact(HASH_SIZE)
                    .next()
                    .map_or_else(
                        || Err::<&[u8], DedupError>(DedupError::SledKeyLengthError),
                        Ok,
                    )?
                    .try_into()?;
                // Check data
                let _: ChunkDbEntry = decrypt(&encrypted_data, &self.chunk_enc_key)?;
            }
            Ok(())
        }

        pub fn new(tree: sled::Tree, chunk_enc_key: EncKey) -> Result<ChunkDb> {
            let cs = ChunkDb {
                state: ChunkDbState{
                    path_gen: FilePathGen::default(),
                    unused_paths: Vec::<PathBuf>::default()
                },
                chunk_map: tree,
                chunk_enc_key: chunk_enc_key,
            };
            cs.self_test()?;
            Ok(cs)
        }

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
                        encrypt(&ChunkDbEntry {
                            file_name: file_name.clone(),
                            ref_count: 1,
                        }, &self.chunk_enc_key)?,
                    )?;

                    Ok((1, file_name))
                }
                Some(old) => {
                    let old: ChunkDbEntry = decrypt(&old, &self.chunk_enc_key)?;

                    let ref_count = old.ref_count + 1;

                    self.chunk_map.insert(
                        key,
                        encrypt(&ChunkDbEntry {
                            file_name: old.file_name.clone(),
                            ref_count,
                        }, &self.chunk_enc_key)?,
                    )?;

                    Ok((ref_count, old.file_name))
                }
            }
        }

        pub fn remove(&mut self, key: &ChunkHash) -> Result<Option<(RefCount, PathBuf)>> {
            /// Removes a chunk reference and returns the reference count as well as the file name the chunk is supposed to be stored in.
            ///
            /// - Returns `Ok(None)` if chunk was not referenced (no file name is associated with that chunk hash)
            /// - Returns `Ok((0, file_name))` if the last reference to this chunk was removed indicating that the chunk file should be removed
            match self.chunk_map.remove(key)? {
                None => Ok(None),
                Some(old) => {
                    let old: ChunkDbEntry = decrypt(&old, &self.chunk_enc_key)?;
                    if old.ref_count <= 1 {
                        // save old file name for reuse
                        self.state.unused_paths.push(old.file_name.clone());
                        Ok(Some((0, old.file_name)))
                    } else {
                        let ref_count = old.ref_count - 1;
                        self.chunk_map.insert(
                            key,
                            encrypt(&ChunkDbEntry {
                                file_name: old.file_name.clone(),
                                ref_count,
                            }, &self.chunk_enc_key)?,
                        )?;
                        Ok(Some((ref_count, old.file_name)))
                    }
                }
            }
        }

        pub fn len(&self) -> usize {
            /// Returns the number of stored chunks
            ///
            /// This performs a full O(n) scan
            self.chunk_map.len()
        }

        pub fn get_mappings(&self) -> Result<BTreeMap<ChunkHash, (RefCount, PathBuf)>> {
            /// Returns a BTreeMap containing the contens of the internal mappings.
            ///
            /// This decrypts all contens creates a compleatly new map in memory
            let mut result = BTreeMap::<ChunkHash, (RefCount, PathBuf)>::new();
            for data in self.chunk_map.iter() {
                let (key, encrypted_data) = data?;
                if key.len() != HASH_SIZE {
                    return Err(DedupError::SledKeyLengthError.into());
                } else {
                    let key: ChunkHash = key
                        .chunks_exact(HASH_SIZE)
                        .next()
                        .map_or_else(
                            || Err::<&[u8], DedupError>(DedupError::SledKeyLengthError),
                            Ok,
                        )?
                        .try_into()?;
                    let chunk_file: ChunkDbEntry = decrypt(&encrypted_data, &self.chunk_enc_key)?;
                    result.insert(key, (chunk_file.ref_count, chunk_file.file_name));
                }
            }
            Ok(result)
        }

        pub fn get_entry(&self, key: &ChunkHash) -> Result<Option<(RefCount, PathBuf)>> {
            match self.chunk_map.get(key)? {
                None => Ok(None),
                Some(encrypted_data) => {
                    let chunk_file: ChunkDbEntry = decrypt(&encrypted_data, &self.chunk_enc_key)?;
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


    fn encrypt<S: Serialize + for<'a> Deserialize<'a>>(data: &S, key: &EncKey) -> Result<Vec<u8>> {
        /// Generic function to encrypt data in dedup
        // generate nonce
        let nonce: EncNonce = XChaCha20Poly1305::generate_nonce(&mut OsRng).into();
        // setup the cipher
        let cipher = XChaCha20Poly1305::new(key.into());
        // convert data to Vec<u8>
        let serialized_data = bincode::serialize(data)?;
        // encrypt the data
        let encrypted_data = cipher.encrypt(&nonce.into(), &serialized_data[..])?;
        // construct CryptoCtx using the nonce and the encrypted data
        let ctx = CryptoCtx {
            nonce,
            data: encrypted_data,
        };
        // convert CryptoCtx to Vec<u8>
        Ok(bincode::serialize(&ctx)?)
    }

    fn decrypt<S: Serialize + for<'a> Deserialize<'a>>(data: &[u8], key: &EncKey) -> Result<S> {
        /// Generic function to decrypt data encrypted by dedup
        // decode encrypted data to split nonce and encrypted data
        let ctx = bincode::deserialize::<CryptoCtx>(data)?;
        // setup the cipher
        let cipher = XChaCha20Poly1305::new(key.into());
        // decrypt the data
        let decrypted_data = cipher.decrypt(&ctx.nonce.into(), &ctx.data[..])?;
        // convert decrypted data to the target data type
        Ok(bincode::deserialize(&decrypted_data)?)
    }

    fn compress_and_encrypt<S: Serialize + for<'a> Deserialize<'a>>(
        data: &S,
        key: &EncKey,
    ) -> Result<Vec<u8>> {
        /// Generic function to compress and encrypt data in dedup
        // generate nonce
        let nonce: EncNonce = XChaCha20Poly1305::generate_nonce(&mut OsRng).into();
        // setup the cipher
        let cipher = XChaCha20Poly1305::new(key.into());
        // convert data to Vec<u8>
        let serialized_data = bincode::serialize(data)?;

        // compress the data
        let mut compressor = DeflateEncoder::new(Vec::new(), Compression::default());
        compressor.write_all(&serialized_data[..])?;
        let compressed_data = compressor.finish()?;
        // encrypt the data
        let encrypted_data = cipher.encrypt(&nonce.into(), &compressed_data[..])?;
        // construct CryptoCtx using the nonce and the encrypted data
        let ctx = CryptoCtx {
            nonce,
            data: encrypted_data,
        };
        // convert CryptoCtx to Vec<u8>
        Ok(bincode::serialize(&ctx)?)
    }

    fn decrypt_and_uncompress<S: Serialize + for<'a> Deserialize<'a>>(
        data: &[u8],
        key: &EncKey,
    ) -> Result<S> {
        /// Generic function to decrypt and uncompress data encrypted by dedup
        // decode encrypted data to split nonce and encrypted data
        let ctx = bincode::deserialize::<CryptoCtx>(data)?;
        // setup the cipher
        let cipher = XChaCha20Poly1305::new(key.into());
        // decrypt the data
        let decrypted_data = cipher.decrypt(&ctx.nonce.into(), &ctx.data[..])?;
        // decompress decrypted data
        let mut uncompressed_data = Vec::new();
        let mut deflater = DeflateDecoder::new(uncompressed_data);
        deflater.write_all(&decrypted_data)?;
        uncompressed_data = deflater.finish()?;
        // deserialize uncompressed data
        Ok(bincode::deserialize(&uncompressed_data)?)
    }

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
    struct InodeDbEntry {
        inode: Inode,
        ref_count: RefCount,
    }


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
                    return Err(DedupError::SledKeyLengthError.into());
                }
                let key: InodeHash = key
                    .chunks_exact(HASH_SIZE)
                    .next()
                    .map_or_else(
                        || Err::<&[u8], DedupError>(DedupError::SledKeyLengthError),
                        Ok,
                    )?
                    .try_into()?;

                // Check data
                let entry: InodeDbEntry = decrypt(&encrypted_data, &self.inode_enc_key)?;
                if key != *entry.inode.keyed_hash(&self.inode_hash_key)?.as_bytes() {
                    return Err(DedupError::SelfTestError.into());
                }
            }
            Ok(())
        }

        pub fn new(tree: sled::Tree, inode_enc_key: EncKey, inode_hash_key: EncKey) -> Result<InodeDb> {
            let db = InodeDb{tree, inode_enc_key, inode_hash_key};
            db.self_test()?;
            Ok(db)
        }

        pub fn len(&self) -> usize {
            self.tree.len()
        }

        pub fn insert(&mut self, inode: Inode) -> Result<(RefCount, InodeHash)> {
            let key: InodeHash = *inode.keyed_hash(&self.inode_hash_key)?.as_bytes();
            match self.tree.remove(key)? {
                Some(old) => {
                    let old: InodeDbEntry = decrypt(&old, &self.inode_enc_key)?;
                    let ref_count = old.ref_count + 1;

                    self.tree.insert(
                        key,
                        encrypt(&InodeDbEntry { inode, ref_count }, &self.inode_enc_key)?,
                    )?;

                    Ok((ref_count, key))
                }
                None => {
                    let encrypted_entry = encrypt(&InodeDbEntry {
                        inode,
                        ref_count: 1,
                    }, &self.inode_enc_key)?;
                    self.tree.insert(key, encrypted_entry)?;
                    Ok((1, key))
                }
            }
        }

        pub fn remove(&mut self, key: &InodeHash) -> Result<Option<(RefCount, Inode)>> {
            match self.tree.remove(key)? {
                None => Ok(None),
                Some(old) => {
                    let old: InodeDbEntry = decrypt(&old, &self.inode_enc_key)?;
                    if old.ref_count <= 1 {
                        Ok(Some((0, old.inode)))
                    } else {
                        let ref_count = old.ref_count - 1;

                        let encrypted_entry = encrypt(&InodeDbEntry {
                            inode: old.inode.clone(),
                            ref_count,
                        }, &self.inode_enc_key)?;
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
                    let entry: InodeDbEntry = decrypt(&encrypted_data, &self.inode_enc_key)?;
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
                    return Err(DedupError::SledKeyLengthError.into());
                } else {
                    let hash: InodeHash = key
                        .chunks_exact(HASH_SIZE)
                        .next()
                        .map_or_else(
                            || Err::<&[u8], DedupError>(DedupError::SledKeyLengthError),
                            Ok,
                        )?
                        .try_into()?;
                    let entry: InodeDbEntry = decrypt(&encrypted_data, &self.inode_enc_key)?;
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

            let enc = compress_and_encrypt(&testdata, &key.into()).unwrap();
            let dec: Chunk = decrypt_and_uncompress(&enc, &key.into()).unwrap();

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
            let mut enc = compress_and_encrypt(&testdata, &key.into()).unwrap();
            let last = enc.pop().unwrap();
            enc.push(!last);

            let dec: Result<Chunk> = decrypt_and_uncompress(&enc, &key.into());
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
            let enc = compress_and_encrypt(&testdata, &key.into()).unwrap();
            key[0] = !key[0];

            let dec: Result<Chunk> = decrypt_and_uncompress(&enc, &key.into());
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

            let enc = encrypt(&testdata, &key.into()).unwrap();
            let dec: Chunk = decrypt(&enc, &key.into()).unwrap();

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
            let mut enc = encrypt(&testdata, &key.into()).unwrap();
            let last = enc.pop().unwrap();
            enc.push(!last);

            let dec: Result<Chunk> = decrypt(&enc, &key.into());
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
            let enc = encrypt(&testdata, &key.into()).unwrap();
            key[0] = !key[0];

            let dec: Result<Chunk> = decrypt(&enc, &key.into());
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
            let key: EncKey = *blake3::hash(b"foobar").as_bytes();
            let config = sled::Config::new().temporary(true);
            let db = config.open().unwrap();

            let mut cs = ChunkDb::new(db.open_tree(b"test").unwrap(), key).unwrap();
            let h1 = blake3::hash(b"foo");
            let h2 = blake3::hash(b"bar");
            let h3 = blake3::hash(b"baz");
            let h4 = blake3::hash(b"foobar");

            assert_eq!(
                cs.insert(h1.as_bytes()).unwrap(),
                (1, PathBuf::from("1.bin"))
            );
            assert_eq!(
                cs.insert(h2.as_bytes()).unwrap(),
                (1, PathBuf::from("2.bin"))
            );
            assert_eq!(
                cs.insert(h3.as_bytes()).unwrap(),
                (1, PathBuf::from("3.bin"))
            );

            assert_eq!(
                cs.insert(h2.as_bytes()).unwrap(),
                (2, PathBuf::from("2.bin"))
            );
            assert_eq!(
                cs.insert(h3.as_bytes()).unwrap(),
                (2, PathBuf::from("3.bin"))
            );

            assert_eq!(
                cs.remove(h1.as_bytes()).unwrap(),
                Some((0, PathBuf::from("1.bin")))
            );
            assert_eq!(cs.remove(h1.as_bytes()).unwrap(), None);

            assert_eq!(
                cs.insert(h4.as_bytes()).unwrap(),
                (1, PathBuf::from("1.bin"))
            );
        }
    }
}
