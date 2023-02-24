use std::collections::HashMap;

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
    inode_db_path:  std::path::PathBuf,
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
