pub mod structs {
    use std::collections::BTreeMap;
    use std::collections::{HashMap, HashSet};
    use std::path::{Path, PathBuf};

    const HASH_SIZE: usize = 32;
    const KEY_SIZE: usize = 32;
    const NONCE_SIZE: usize = 24;

    type ChunkHash = [u8; HASH_SIZE];
    type InodeHash = [u8; HASH_SIZE];
    //type BackupHash = [u8; HASH_SIZE];
    type EncNonce = [u8; NONCE_SIZE];
    type EncKey = [u8; KEY_SIZE];
    type HashKey = [u8; KEY_SIZE];
    type SigKey = [u8; KEY_SIZE];

    #[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
    struct Metadata {
        mode: u32,
        uid: u32,
        gid: u32,
        atime: i64,
        mtime: i64,
        ctime: i64,
    }

    #[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
    struct ChunkerConf {
        minimum_chunk_size: usize,
        average_chunk_size: usize,
        maximum_chunk_size: usize,
    }

    #[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
    struct Manifest {
        salt: [u8; 32],
        chunk_root_dir: std::path::PathBuf,
        backup_db_path: std::path::PathBuf,
        inode_db_path: std::path::PathBuf,
        version: String,
        chunker_conf: ChunkerConf,
    }

    #[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
    struct RuntimeConf {
        manifest_sig_key: SigKey,
        chunk_encryption_key: EncKey,
        chunk_hash_key: HashKey,
        inode_encryption_key: EncKey,
        inode_hash_key: HashKey,
        backup_encryption_key: EncKey,
    }

    type EncBackups = BTreeMap<EncNonce, Vec<u8>>;

    #[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
    struct Backup {
        timestamp: String,
        name: String,
        root: Directory,
    }

    #[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
    struct Symlink {
        relpath: std::path::PathBuf,
        target: std::path::PathBuf,
        metadata: Metadata,
    }

    #[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
    struct Directory {
        relpath: std::path::PathBuf,
        metadata: Metadata,
        contents: Vec<InodeHash>,
    }

    #[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
    struct File {
        relpath: std::path::PathBuf,
        chunk_ids: Vec<ChunkHash>,
        metadata: Metadata,
    }

    #[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
    enum Inode {
        File,
        Directory,
        Symlink,
    }

    type EncInodes = HashMap<InodeHash, (EncNonce, Vec<u8>)>;

    #[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
    struct EncChunk {
        hash: ChunkHash,
        nonce: EncNonce,
        chunk: Chunk,
    }

    #[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
    struct Chunk {
        data: Vec<u8>,
    }

    pub fn log2u64(x: u64) -> Option<u64> {
        if x > 0 {
            Some(std::mem::size_of::<u64>() as u64 * 8u64 - x.leading_zeros() as u64 - 1u64)
        } else {
            None
        }
    }

    #[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
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

    #[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
    pub struct ChunkFile {
        ref_count: u64,
        file_name: std::path::PathBuf,
    }

    impl ChunkFile {
        fn fromPathBuf(file_name: PathBuf) -> ChunkFile {
            ChunkFile {
                ref_count: 1u64,
                file_name: file_name,
            }
        }
        fn new(file_name: PathBuf, ref_count: u64) -> ChunkFile {
            ChunkFile {
                ref_count: ref_count,
                file_name: file_name,
            }
        }
        fn ref_count(&self) -> u64 {
            return self.ref_count;
        }
        fn file_name(&self) -> PathBuf {
            return self.file_name.clone();
        }
    }

    #[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
    pub struct CryptoCtx {
        nonce: EncNonce,
        data: Vec<u8>,
    }

    use std::{error, fmt};

    #[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
    enum DedupError {
        SledKeyLengthError,
    }

    impl fmt::Display for DedupError {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            match self {
                DedupError::SledKeyLengthError => {
                    write!(f, "SledKeyLengthError: a sled key seems to be of wrong length")
                }
            }
        }
    }

    impl error::Error for DedupError {}


    #[derive(Debug)]
    enum Error {
        CryptoError(chacha20poly1305::aead::Error),
        DedupError(DedupError),
        SledError(sled::Error),
        BincodeError(bincode::ErrorKind),
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
            }
        }
    }

    impl error::Error for Error {}

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

    type Result<T> = std::result::Result<T, Error>;

    use chacha20poly1305::{
        aead::{Aead, AeadCore, KeyInit, OsRng},
        Nonce, XChaCha20Poly1305,
    };
    use once_cell::sync::OnceCell;

    pub static CHUNK_STORE_KEY: OnceCell<EncKey> = OnceCell::new();

    use sled::transaction::ConflictableTransactionError;

    #[derive(Debug)]
    struct ChunkStore {
        path_gen: FilePathGen,
        unused_paths: Vec<PathBuf>,
        chunk_map: sled::Tree,
    }

    impl ChunkStore {
        fn self_test(&self) -> Result<()> {
            /// Check the ChunkStore contents for errors
            ///
            /// This is a O(n) operation

            for data in self.chunk_map.iter() {
                let (key, encrypted_data) = data?;
                // Check key
                if key.len() != 32 {
                    return Err(DedupError::SledKeyLengthError.into());
                }
                // Check data
                let _ = decrypt_chunk_file(&encrypted_data)?;
            }
            Ok(())
        }

        fn new(tree: sled::Tree) -> Result<ChunkStore> {
            let cs = ChunkStore {
                path_gen: FilePathGen::default(),
                unused_paths: Vec::<PathBuf>::default(),
                chunk_map: tree,
            };
            cs.self_test()?;
            Ok(cs)
        }

        fn insert(&mut self, key: &ChunkHash) -> Result<(u64, PathBuf)> {
            match self
                .chunk_map
                .remove(key)?
            {
                None => {
                    let file_name = self.unused_paths
                                         .pop()
                                         .unwrap_or_else(||{
                                             PathBuf::from(self.path_gen.next()
                                                           .expect("BUG: Please contact me if you need more than 10^19 chunks, I'd really like to know the system you are on"))
                    });

                    self.chunk_map
                        .insert(
                            key,
                            encrypt_chunk_file(&ChunkFile::fromPathBuf(file_name.clone()))?,
                        )?;

                    Ok((1u64, file_name))
                }
                Some(old) => {
                    let old = decrypt_chunk_file(&old)?;

                    let ref_count = old.ref_count + 1;

                    self.chunk_map
                        .insert(
                            key,
                            encrypt_chunk_file(&ChunkFile::new(old.file_name.clone(), ref_count))?,
                        )?;

                    Ok((ref_count, old.file_name))
                }
            }
        }

        fn remove(&mut self, key: &ChunkHash) -> Result<Option<(u64, PathBuf)>> {
            /// Removes a chunk reference and returns the reference count as well as the file name the chunk is supposed to be stored in.
            ///
            /// - Returns `Ok(None)` if chunk was not referenced (no file name is associated with that chunk hash)
            /// - Returns `Ok((0, file_name))` if the last reference to this chunk was removed indicating that the chunk file should be removed
            match self
                .chunk_map
                .remove(key)?
            {
                None => Ok(None),
                Some(old) => {
                    let old = decrypt_chunk_file(&old)?;
                    if old.ref_count <= 1 {
                        // save old file name for reuse
                        self.unused_paths.push(old.file_name.clone());
                        Ok(Some((0u64, old.file_name)))
                    } else {
                        let ref_count = old.ref_count - 1;
                        self.chunk_map
                            .insert(
                                key,
                                encrypt_chunk_file(&ChunkFile::new(
                                    old.file_name.clone(),
                                    ref_count,
                                ))?,
                            )?;
                        Ok(Some((ref_count, old.file_name)))
                    }
                }
            }
        }

        fn len(&self) -> usize {
            /// Returns the number of stored chunks
            ///
            /// This performs a full O(n) scan
            self.chunk_map.len()
        }

        fn get_mappings(&self) -> Result<BTreeMap<ChunkHash, ChunkFile>> {
            /// Returns a BTreeMap containing the contens of the internal mappings.
            ///
            /// This decrypts all contens creates a compleatly new map in memory
            let mut result = BTreeMap::<ChunkHash, ChunkFile>::new();
            for data in self.chunk_map.iter() {
                let (key, encrypted_data) = data?;
                if key.len() != HASH_SIZE {
                    return Err(DedupError::SledKeyLengthError.into());
                } else {
                    let key: ChunkHash = key.chunks_exact(HASH_SIZE).next().unwrap().try_into().unwrap();
                    let chunk_file = decrypt_chunk_file(&encrypted_data)?;
                    result.insert(key, chunk_file);
                }
            }
            Ok(result)
        }

        fn get_chunk_file(&self, key: &ChunkHash) -> Result<Option<ChunkFile>> {
            match self
                .chunk_map
                .get(key)?
            {
                None => Ok(None),
                Some(encrypted_data) => {
                    let chunk_file = decrypt_chunk_file(&encrypted_data)?;
                    Ok(Some(chunk_file))
                }
            }
        }
        fn get_ref_count(&self, key: &ChunkHash) -> Result<u64> {
            match self.get_chunk_file(key)? {
                None => Ok(0u64),
                Some(chunk_file) => Ok(chunk_file.ref_count),
            }
        }

        fn get_file_name(&self, key: &ChunkHash) -> Result<Option<PathBuf>> {
            match self.get_chunk_file(key)? {
                None => Ok(None),
                Some(chunk_file) => Ok(Some(chunk_file.file_name)),
            }
        }
    }

    fn encrypt_chunk_file(chunk_file: &ChunkFile) -> Result<Vec<u8>> {
        encrypt(
            chunk_file,
            CHUNK_STORE_KEY
                .get()
                .expect("CHUNK_STORE_KEY should be initialized!"),
        )
    }

    fn decrypt_chunk_file(encrypted_data: &[u8]) -> Result<ChunkFile> {
        decrypt(
            encrypted_data,
            CHUNK_STORE_KEY
                .get()
                .expect("CHUNK_STORE_KEY should be initialized!"),
        )
    }

    use serde::{Deserialize, Serialize};
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


    #[derive(Debug)]
    struct InodeDb(sled::Tree);

    impl InodeDb{
        fn self_test(&self) -> Result<()> {
            todo!()
        }

        fn new(&mut self, tree: sled::Tree) -> Result<InodeDb>{
            todo!()
        }

        fn insert(&mut self, key: InodeHash, inode: Inode) -> Result<()> {
            todo!()
        }

        fn remove(&mut self, key: InodeHash) -> Result<Inode> {
            todo!()
        }

        fn get_inode(&self, key: InodeHash) -> Result<Inode> {
            todo!()
        }

        fn get_mappings(&self) -> Result<BTreeMap<InodeHash, Inode>> {
            todo!()
        }
    }

    #[cfg(test)]
    mod tests {
        // Note this useful idiom: importing names from outer (for mod tests) scope.
        use super::*;

        #[test]
        fn test_encryption_success() {
            let testdata = Chunk {
                data: vec![1u8, 2u8, 3u8, 4u8, 5u8],
            };
            let key = XChaCha20Poly1305::generate_key(&mut OsRng);
            let enc = encrypt(&testdata, &key.into()).unwrap();
            let dec: Chunk = decrypt(&enc, &key.into()).unwrap();
            assert_eq!(testdata, dec);
        }

        #[test]
        fn test_encryption_fail_tempering() {
            let testdata = Chunk {
                data: vec![1u8, 2u8, 3u8, 4u8, 5u8],
            };
            let key = XChaCha20Poly1305::generate_key(&mut OsRng);
            let mut enc = encrypt(&testdata, &key.into()).unwrap();
            let last = enc.pop().unwrap();
            enc.push(!last);

            let dec: Result<Chunk> = decrypt(&enc, &key.into());
            assert!(dec.is_err());
        }

        #[test]
        fn test_encryption_fail_key() {
            let testdata = Chunk {
                data: vec![1u8, 2u8, 3u8, 4u8, 5u8],
            };
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
        fn test_ChunkStore() {

            let key: EncKey = *blake3::hash(b"foobar").as_bytes();
            CHUNK_STORE_KEY.set(key);
            let config = sled::Config::new().temporary(true);
            let db = config.open().unwrap();

            let mut cs = ChunkStore::new(db.open_tree(b"test").unwrap()).unwrap();
            let h1 = blake3::hash(b"foo");
            let h2 = blake3::hash(b"bar");
            let h3 = blake3::hash(b"baz");
            let h4 = blake3::hash(b"foobar");

            assert_eq!(cs.insert(h1.as_bytes()).unwrap(), (1, PathBuf::from("1.bin")));
            assert_eq!(cs.insert(h2.as_bytes()).unwrap(), (1, PathBuf::from("2.bin")));
            assert_eq!(cs.insert(h3.as_bytes()).unwrap(), (1, PathBuf::from("3.bin")));

            assert_eq!(cs.insert(h2.as_bytes()).unwrap(), (2, PathBuf::from("2.bin")));
            assert_eq!(cs.insert(h3.as_bytes()).unwrap(), (2, PathBuf::from("3.bin")));

            assert_eq!(cs.remove(h1.as_bytes()).unwrap(), Some((0, PathBuf::from("1.bin"))));
            assert_eq!(cs.remove(h1.as_bytes()).unwrap(), None);

            assert_eq!(cs.insert(h4.as_bytes()).unwrap(), (1, PathBuf::from("1.bin")));
        }
    }
}
