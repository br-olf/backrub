pub mod structs {
    use std::collections::BTreeMap;
    use std::collections::HashMap;
    use std::path::{PathBuf,Path};

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

    type EncBackups = HashMap<EncNonce, Vec<u8>>;

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

    #[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
    pub struct FilePathGen {
        count: u64,
    }

    impl Iterator for FilePathGen {
        type Item = String;

        fn next(&mut self) -> std::option::Option<<Self as Iterator>::Item> {
            let mut name = String::new();
            // we need floor(log2(self.count)/8) folders and 1 byte for the file_name
            //folders are
            let num_bytes = self.count as f64;
            let num_bytes = num_bytes.log2() / 8f64;
            let num_bytes = num_bytes.floor() as usize + 1 as usize;
            for i in 1..num_bytes {
                // b0 = 0xff
                let mut b0: u64 = !0u8 as u64;
                // shift b0 in position
                b0 = b0 << 8 * i;
                // apply mask
                b0 = self.count & b0;
                // shift back
                let b0 = (b0 >> (8 * i)) as u8;
                //println!("{}", b0);
                name += &format!("{b0:x}");
                name += "/";
            }

            let mut b0 = !0u8 as u64;
            b0 = self.count as u64 & b0;
            let b0 = b0 as u8;
            name += &format!("{b0:x}.bin");
            self.count += 1;
            return Some(name);
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
    pub struct ChunkStore(BTreeMap<ChunkHash, ChunkFile>);

    impl ChunkStore {
        fn insert(&mut self, key: &ChunkHash, filename: &Path) -> Option<ChunkFile> {
                self.0.insert(key.clone(), ChunkFile::new(filename.to_path_buf(), self.get_ref_count(key) + 1))
        }

        fn remove(&mut self, key: &ChunkHash) -> Option<ChunkFile> {
            let ref_count = self.get_ref_count(key);
            if ref_count > 1 {
                return self.0.insert(key.clone(), ChunkFile::new(self.get_filename(key).expect("Should never fail!").to_path_buf(), ref_count-1));
            } else {
                return self.0.remove(key);
            }
        }

        fn get_filename(&self, key: &ChunkHash) -> Option<&Path>{
            if let Some(e) = self.get_chunk_file(key) {
                Some(e.filename.as_path())
            } else {
                None
            }
        }

        fn get_ref_count(&self, key: &ChunkHash) -> u64{
            let mut ref_count = 0u64;
            if let Some(e) = self.0.get(key) {
                ref_count = e.ref_count();
            }
            ref_count
        }
        fn get_chunk_file(&self, key: &ChunkHash) -> Option<&ChunkFile> {
            self.0.get(key)
        }
    }
}
