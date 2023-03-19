pub mod structs {
    use std::collections::BTreeMap;
    use std::collections::{HashMap, HashSet};
    use std::path::{Path, PathBuf};

    type ChunkHash = [u8; 32];
    type InodeHash = [u8; 32];
    //type BackupHash = [u8; 32];
    type EncNonce = [u8; 24];
    type EncKey = [u8; 32];
    type HashKey = [u8; 32];
    type SigKey = [u8; 32];

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
        filename: std::path::PathBuf,
    }

    impl ChunkFile {
        fn fromPathBuf(filename: PathBuf) -> ChunkFile {
            return ChunkFile {
                ref_count: 1u64,
                filename: filename,
            };
        }
        fn new(filename: PathBuf, ref_count: u64) -> ChunkFile {
            return ChunkFile {
                ref_count: ref_count,
                filename: filename,
            };
        }
        fn ref_count(&self) -> u64 {
            return self.ref_count;
        }
        fn filename(&self) -> PathBuf {
            return self.filename.clone();
        }
    }

    #[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
    pub struct ChunkFileBTreeMap(BTreeMap<ChunkHash, ChunkFile>);

    impl ChunkFileBTreeMap {
        fn remove(&mut self, key: &ChunkHash) -> Option<(u64, PathBuf)> {
            let ref_count = self.get_ref_count(key);
            match ref_count {
                0 => None,
                1 => Some((0, self.0.remove(key)?.filename)),
                _ => {
                    let ref_count = ref_count - 1;
                    let filename = self
                        .get_filename(key)
                        .expect("Should never fail!")
                        .to_path_buf();
                    self.0
                        .insert(*key, ChunkFile::new(filename.clone(), ref_count));
                    Some((ref_count, filename))
                }
            }
        }

        fn insert(&mut self, key: &ChunkHash, filename: PathBuf) -> Option<ChunkFile> {
            self.0.insert(
                key.clone(),
                ChunkFile::new(filename, self.get_ref_count(key) + 1),
            )
        }
    }

    pub trait ChunkFileMap {
        fn get_filename(&self, key: &ChunkHash) -> Option<&Path>;
        fn get_ref_count(&self, key: &ChunkHash) -> u64;
        fn get_chunk_file(&self, key: &ChunkHash) -> Option<&ChunkFile>;
        fn get_mappings(&self) -> &BTreeMap<ChunkHash, ChunkFile>;
        fn len(&self) -> usize;
    }

    impl ChunkFileMap for ChunkFileBTreeMap {
        fn len(&self) -> usize {
            self.0.len()
        }
        fn get_mappings(&self) -> &BTreeMap<ChunkHash, ChunkFile> {
            &self.0
        }

        fn get_filename(&self, key: &ChunkHash) -> Option<&Path> {
            if let Some(e) = self.get_chunk_file(key) {
                Some(e.filename.as_path())
            } else {
                None
            }
        }

        fn get_ref_count(&self, key: &ChunkHash) -> u64 {
            if let Some(e) = self.0.get(key) {
                e.ref_count()
            } else {
                0u64
            }
        }

        fn get_chunk_file(&self, key: &ChunkHash) -> Option<&ChunkFile> {
            self.0.get(key)
        }
    }

    #[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
    struct ChunkStore {
        path_gen: FilePathGen,
        chunkmap: ChunkFileBTreeMap,
        unused_paths: Vec<PathBuf>,
    }

    impl ChunkStore {
        fn new() -> ChunkStore {
            ChunkStore::default()
        }
        fn insert(&mut self, key: &ChunkHash) -> (u64, PathBuf) {
            let mut filename = PathBuf::default();
            let ref_count = self.chunkmap.get_ref_count(key) + 1;
            if let Some(name) = self.chunkmap.get_filename(key) {
                filename = name.to_path_buf();
            } else {
                filename = self
                    .unused_paths
                    .pop()
                    .unwrap_or_else(|| match self.path_gen.next() {
                        Some(name) => PathBuf::from(name),
                        None => {
                            todo!("Error Handling: Please contact me if use more than 10^19 chunks, I would really like to know the system you are on")
                        }
                    })
            }
            self.chunkmap.insert(key, filename.clone());
            (ref_count, filename)
        }
        fn remove(&mut self, key: &ChunkHash) -> Option<(u64, PathBuf)> {
            let (ref_count, filename) = self.chunkmap.remove(key)?;
            if ref_count == 0 {
                self.unused_paths.push(filename.clone())
            }
            Some((ref_count, filename))
        }
        fn get_unused(&self) -> &Vec<PathBuf> {
            &self.unused_paths
        }
    }

    impl ChunkFileMap for ChunkStore {
        fn len(&self) -> usize {
            self.chunkmap.len()
        }
        fn get_mappings(&self) -> &BTreeMap<ChunkHash, ChunkFile> {
            self.chunkmap.get_mappings()
        }
        fn get_filename(&self, key: &ChunkHash) -> Option<&Path> {
            self.chunkmap.get_filename(key)
        }
        fn get_ref_count(&self, key: &ChunkHash) -> u64 {
            self.chunkmap.get_ref_count(key)
        }
        fn get_chunk_file(&self, key: &ChunkHash) -> Option<&ChunkFile> {
            self.chunkmap.get_chunk_file(key)
        }
    }

    #[cfg(test)]
    mod tests {
        // Note this useful idiom: importing names from outer (for mod tests) scope.
        use super::*;

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
        }
        #[test]
        fn test_ChunkStore() {
            let mut cs = ChunkStore::new();
            let h1 = blake3::hash(b"foo");
            let h2 = blake3::hash(b"bar");
            let h3 = blake3::hash(b"baz");
            let h4 = blake3::hash(b"foobar");

            assert_eq!(cs.insert(h1.as_bytes()), (1, PathBuf::from("1.bin")));
            assert_eq!(cs.insert(h2.as_bytes()), (1, PathBuf::from("2.bin")));
            assert_eq!(cs.insert(h3.as_bytes()), (1, PathBuf::from("3.bin")));

            assert_eq!(cs.insert(h2.as_bytes()), (2, PathBuf::from("2.bin")));
            assert_eq!(cs.insert(h3.as_bytes()), (2, PathBuf::from("3.bin")));

            assert_eq!(cs.remove(h1.as_bytes()), Some((0, PathBuf::from("1.bin"))));
            assert_eq!(cs.remove(h1.as_bytes()), None);

            assert_eq!(cs.insert(h4.as_bytes()), (1, PathBuf::from("1.bin")));
        }
    }

    #[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
    pub struct CryptoCtx {
        nonce: EncNonce,
        data: Vec<u8>,
    }

    use chacha20poly1305::{
        aead::{Aead, AeadCore, KeyInit, OsRng},
        Nonce, XChaCha20Poly1305,
    };
    use once_cell::sync::OnceCell;

    pub static CHUNK_STORE_KEY: OnceCell<EncKey> = OnceCell::new();

    fn insert_chunk_to_sled(old: Option<&[u8]>) -> Option<Vec<u8>> {
        match old {
            Some(old) => match bincode::deserialize::<CryptoCtx>(old) {
                Ok(old) => {
                    let cipher = XChaCha20Poly1305::new(CHUNK_STORE_KEY.get().unwrap().into());
                    match cipher.decrypt(&old.nonce.into(), &old.data[..]) {
                        Ok(pt) => match bincode::deserialize::<ChunkFile>(&pt[..]) {
                            Ok(old) => {
                                todo!()
                            }
                            Err(e) => {
                                todo!()
                            }
                        },
                        Err(e) => {
                            todo!()
                        }
                    }
                }
                Err(e) => {
                    todo!()
                }
            },
            None => {
                todo!()
            }
        }
        /*
                let mut filename = PathBuf::default();
                let ref_count = self.get_ref_count(key) + 1;
                if let Some(name) = self.chunkmap.get_filename(key) {
                    filename = name.to_path_buf();
                } else {
                    filename = self
                        .unused_paths
                        .pop()
                        .unwrap_or_else(|| match self.path_gen.next() {
                            Some(name) => PathBuf::from(name),
                            None => {
                                todo!("Error Handling: Please contact me if use more than 10^19 chunks, I would really like to know the system you are on")
                            }
                        })
                }
                self.chunkmap.insert(key, filename.clone());
                (ref_count, filename)
        */
    }

    /*
    //    #[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
        struct ChunkStoreSled {
            path_gen: FilePathGen,
            chunkmap: sled::Tree,
            unused_paths: Vec<PathBuf>,
        }


        impl ChunkStoreSled {
            fn new() -> ChunkStoreSled {
                ChunkStoreSled::default()
            }
            fn insert(&mut self, key: &ChunkHash) -> (u64, PathBuf) {
                let mut filename = PathBuf::default();
                let ref_count = self.get_ref_count(key) + 1;
                if let Some(name) = self.chunkmap.get_filename(key) {
                    filename = name.to_path_buf();
                } else {
                    filename = self
                        .unused_paths
                        .pop()
                        .unwrap_or_else(|| match self.path_gen.next() {
                            Some(name) => PathBuf::from(name),
                            None => {
                                todo!("Error Handling: Please contact me if use more than 10^19 chunks, I would really like to know the system you are on")
                            }
                        })
                }
                self.chunkmap.insert(key, filename.clone());
                (ref_count, filename)
            }
            fn remove(&mut self, key: &ChunkHash) -> Option<(u64, PathBuf)> {
                let (ref_count, filename) = self.chunkmap.remove(key)?;
                if ref_count == 0 {
                    self.unused_paths.push(filename.clone())
                }
                Some((ref_count, filename))
            }
            fn get_unused(&self) -> &Vec<PathBuf> {
                &self.unused_paths
            }
        }

        impl ChunkFileMap for ChunkStoreSled {
            fn len(&self) -> usize {
                self.chunkmap.len()
            }
            fn get_mappings(&self) -> &BTreeMap<ChunkHash, ChunkFile> {
                self.chunkmap.get_mappings()
            }
            fn get_filename(&self, key: &ChunkHash) -> Option<&Path> {
                self.chunkmap.get_filename(key)
            }
            fn get_ref_count(&self, key: &ChunkHash) -> u64 {
                self.chunkmap.get_ref_count(key)
            }
            fn get_chunk_file(&self, key: &ChunkHash) -> Option<&ChunkFile> {
                self.chunkmap.get_chunk_file(key)
            }
        }
    */
}
