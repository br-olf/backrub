use std::{
    env, fs,
    io::prelude::*,
    path::{Path, PathBuf},
};

use async_std::{
    fs::{DirEntry, File},
    sync::Mutex,
};
use chacha20poly1305::aead::{rand_core::RngCore, OsRng};
use futures::executor::block_on;
use serde::{Deserialize, Serialize};

use crate::utils::chunk_and_hash;

use super::db::*;
use super::error::*;
use super::structs::*;
use super::traits::*;
use super::*;

const KB: u64 = 1024;
const MB: u64 = 1024 * KB;
const GB: u64 = 1024 * MB;

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
            minimum_chunk_size: 4 * MB,
            average_chunk_size: 16 * MB,
            maximum_chunk_size: 64 * MB,
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
    inode_db: InodeDb,
    chunk_db: ChunkDb,
    manifest: Manifest,
    keys: CryptoKeys,
    sig_key: Key256,
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
        let sig_key = Key256::try_from(sig_key.as_slice())?;

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
            inode_db: inode_db,
            chunk_db: chunk_db,
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
        let sig_key = Key256::try_from(sig_key.as_slice())?;

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
            inode_db: inode_db,
            chunk_db: chunk_db,
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
        // copy manifest
        let mut manifest = self.manifest.clone();

        // update manifest
        manifest.chunk_db_state = self.chunk_db.state.clone();

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

        self.backup_dir(path, conf)?;

        todo!("create backup db entry")
    }

    /// performs all backup operations for a directory
    fn backup_dir(&mut self, path: &Path, conf: &BackupConf) -> Result<(Vec<Inode>)> {
        let dir_iter = fs::read_dir(path)?;

        let mut result = Vec::<Inode>::new();

        struct DEntry {
            path: PathBuf,
            meta: fs::Metadata,
        }

        let mut dirs = Vec::<PathBuf>::new();
        let mut files = Vec::<DEntry>::new();
        let mut slinks = Vec::<DEntry>::new();

        for entry in dir_iter {
            let entry = entry?;
            let e_meta = entry.metadata()?;
            let r_path = entry.path();

            if e_meta.is_dir() {
                dirs.push(r_path)
            } else if e_meta.is_file() {
                files.push(DEntry {
                    path: r_path,
                    meta: e_meta,
                })
            } else if e_meta.is_symlink() {
                slinks.push(DEntry {
                    path: r_path,
                    meta: e_meta,
                })
            }
        }

        for link in slinks {
            result.push(Inode::Symlink(Symlink {
                relpath: link.path.clone(),
                target: fs::read_link(link.path)?.to_path_buf(),
                metadata: structs::Metadata::from(link.meta),
            }));
        }

        use memmap::Mmap;
        use std::fs::File;

        for file in files {
            let f = File::open(file.path.clone()).unwrap();
            let mmap = unsafe { Mmap::map(&f).unwrap() };
            match chunk_and_hash(
                &mmap,
                &self.manifest.chunker_conf,
                &self.keys.chunk_hash_key,
                &self.keys.inode_hash_key,
            ){
                Ok(res) => {
                    let (chunks, hash) = res;
                    result.push(Inode::File(structs::File{
                        relpath: file.path.clone(),
                        metadata: structs::Metadata::from(file.meta),
                        file_hash: Hash256::from(hash.as_bytes()),
                        chunk_ids: chunks.into_iter().map(|(c, h)| Hash256::from(h.as_bytes())).collect(),
                    }));

                    for (d, h) in chunks.into_iter(){
                        match self.chunk_db.insert(&Hash256::from(h.as_bytes())) {
                            Ok((count, path)) => {
                                if count == 1 {
                                    todo!("create chunk file on {:?} with data {:?}",path,d)
                                }
                            }

                            Err(e) => { todo!("report error: {:?}", e) }
                        }
                    }
                }
                Err(e) => { todo!("report error: {:?}",e) }
            }
        }

        for dir in dirs{
            match self.backup_dir(dir.as_path(), conf) {
                Ok(mut inodes) => { result.append(&mut inodes) }
                Err(e) => { todo!("report error: {:?}",e) }
            }
        }
        todo!()
    }
}

use std::sync::Arc;

#[derive(Debug)]
pub enum TDirEntry {
    Dir(TDir),
    Link(TLink),
    File(TFile),
}
#[derive(Debug)]
pub struct TDir {
    pub path: PathBuf,
    pub meta: fs::Metadata,
    pub cont: Vec<Arc<TDirEntry>>,
}

#[derive(Debug)]
pub struct TLink {
    pub path: PathBuf,
    pub meta: fs::Metadata,
}

#[derive(Debug)]
pub struct TFile {
    pub path: PathBuf,
    pub meta: fs::Metadata,
}

pub fn index_dir(
    path: &Path,
) -> Result<(Arc<TDirEntry>, Vec<Arc<TDirEntry>>, Vec<Arc<TDirEntry>>)> {
    let dir_iter = fs::read_dir(path)?;

    let mut dir_contents = Vec::<Arc<TDirEntry>>::new();
    let mut all_files = Vec::<Arc<TDirEntry>>::new();
    let mut all_contents = Vec::<Arc<TDirEntry>>::new();

    for entry in dir_iter {
        let entry = entry?;
        let e_meta = entry.metadata()?;
        let e_path = entry.path();

        if e_meta.is_dir() {
            let (d, mut a, mut f) = index_dir(&e_path)?;
            dir_contents.push(d);
            all_contents.append(&mut a);
            all_files.append(&mut f);
        } else if e_meta.is_file() {
            let e = Arc::from(TDirEntry::File(TFile {
                path: e_path,
                meta: e_meta,
            }));
            dir_contents.push(e.clone());
            all_contents.push(e.clone());
            all_files.push(e);
        } else if e_meta.is_symlink() {
            let e = Arc::from(TDirEntry::Link(TLink {
                path: e_path,
                meta: e_meta,
            }));
            dir_contents.push(e.clone());
            all_contents.push(e);
        }
    }

    let res = Arc::from(TDirEntry::Dir(TDir {
        path: path.to_path_buf(),
        meta: fs::metadata(path)?,
        cont: dir_contents,
    }));

    all_contents.push(res.clone());
    Ok((res, all_contents, all_files))
}
