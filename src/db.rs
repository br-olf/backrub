use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, marker::PhantomData, path::PathBuf};

use super::error::*;
use super::structs::*;
use super::traits::*;

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
    pub(crate) chunk_map: sled::Tree,
    pub(crate) state: ChunkDbState,
    pub(crate) chunk_enc_key: Key256,
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
            let _: Hash256 = key
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
        chunk_enc_key: Key256,
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
    pub fn new(tree: sled::Tree, chunk_enc_key: Key256) -> Result<ChunkDb> {
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

    /// Inserts a new [`Hash256`] into the database and returns a tuple [`(RefCount, PathBuf)`] of the reference count and the file name the chunk should be stored in
    pub fn insert(&mut self, key: &Hash256) -> Result<(RefCount, PathBuf)> {
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
    pub fn remove(&mut self, key: &Hash256) -> Result<Option<(RefCount, PathBuf)>> {
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
    pub fn get_mappings(&self) -> Result<BTreeMap<Hash256, (RefCount, PathBuf)>> {
        let mut result = BTreeMap::<Hash256, (RefCount, PathBuf)>::new();
        for data in self.chunk_map.iter() {
            let (key, encrypted_data) = data?;
            if key.len() != HASH_SIZE {
                return Err(BackrubError::SledKeyLengthError.into());
            } else {
                let key: Hash256 = key
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

    pub fn get_entry(&self, key: &Hash256) -> Result<Option<(RefCount, PathBuf)>> {
        match self.chunk_map.get(key)? {
            None => Ok(None),
            Some(encrypted_data) => {
                let chunk_file = ChunkDbEntry::decrypt(&encrypted_data, &self.chunk_enc_key)?;
                Ok(Some((chunk_file.ref_count, chunk_file.file_name)))
            }
        }
    }

    pub fn get_ref_count(&self, key: &Hash256) -> Result<Option<RefCount>> {
        match self.get_entry(key)? {
            None => Ok(None),
            Some((ref_count, _file_name)) => Ok(Some(ref_count)),
        }
    }

    pub fn get_file_name(&self, key: &Hash256) -> Result<Option<PathBuf>> {
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
    fn keyed_hash(&self, key: &Key256) -> Result<blake3::Hash> {
        self.data.keyed_hash(key)
    }
}

/// Generic encrypted reference countig database with [sled] backend
#[derive(Debug)]
pub struct RcDb<T: Hashable + Serialize + for<'a> Deserialize<'a>> {
    tree: sled::Tree,
    data_enc_key: Key256,
    data_hash_key: Key256,
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
            let key: Hash256 = key
                .chunks_exact(HASH_SIZE)
                .next()
                .map_or_else(
                    || Err::<&[u8], BackrubError>(BackrubError::SledKeyLengthError),
                    Ok,
                )?
                .try_into()?;

            // Check data
            let entry = RcDbEntry::<T>::decrypt(&encrypted_data, &self.data_enc_key)?;
            if key != Hash256::from(*entry.data.keyed_hash(&self.data_hash_key)?.as_bytes()) {
                return Err(BackrubError::SelfTestError.into());
            }
        }
        Ok(())
    }

    /// Creates a new reference counting database from a `sled::Tree` and running a self test
    pub fn new(tree: sled::Tree, data_enc_key: Key256, data_hash_key: Key256) -> Result<RcDb<T>> {
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
    pub fn insert(&mut self, data: T) -> Result<(RefCount, Hash256)> {
        let key = Hash256::from(*data.keyed_hash(&self.data_hash_key)?.as_bytes());
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
    pub fn remove(&mut self, key: &Hash256) -> Result<Option<(RefCount, T)>> {
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
    pub fn purge(&mut self, key: &Hash256) -> Result<Option<(RefCount, T)>> {
        match self.tree.remove(key)? {
            None => Ok(None),
            Some(old) => {
                let old = RcDbEntry::decrypt(&old, &self.data_enc_key)?;
                Ok(Some((old.ref_count, old.data)))
            }
        }
    }

    /// Gets the current refercence count and data
    pub fn get_data_db_entry(&self, key: &Hash256) -> Result<Option<(RefCount, T)>> {
        match self.tree.get(key)? {
            None => Ok(None),
            Some(encrypted_data) => {
                let entry = RcDbEntry::decrypt(&encrypted_data, &self.data_enc_key)?;
                Ok(Some((entry.ref_count, entry.data)))
            }
        }
    }

    /// Gets only the referenced data
    pub fn get_data(&self, key: &Hash256) -> Result<Option<T>> {
        match self.get_data_db_entry(key)? {
            None => Ok(None),
            Some((_ref_count, data)) => Ok(Some(data)),
        }
    }

    /// Gets only the reference count
    pub fn get_ref_count(&self, key: &Hash256) -> Result<Option<RefCount>> {
        match self.get_data_db_entry(key)? {
            None => Ok(None),
            Some((ref_count, _data)) => Ok(Some(ref_count)),
        }
    }

    /// Returns a complete in memory representation of the database
    pub fn get_mappings(&self) -> Result<BTreeMap<Hash256, (RefCount, T)>> {
        let mut result = BTreeMap::<Hash256, (RefCount, T)>::new();
        for data in self.tree.iter() {
            let (key, encrypted_data) = data?;
            if key.len() != HASH_SIZE {
                return Err(BackrubError::SledKeyLengthError.into());
            } else {
                let hash: Hash256 = key
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
    inode_enc_key: Key256,
    inode_hash_key: Key256,
}

impl InodeDb {
    pub fn self_test(&self) -> Result<()> {
        for data in self.tree.iter() {
            let (key, encrypted_data) = data?;

            // Check Key
            if key.len() != HASH_SIZE {
                return Err(BackrubError::SledKeyLengthError.into());
            }
            let key: Hash256 = key
                .chunks_exact(HASH_SIZE)
                .next()
                .map_or_else(
                    || Err::<&[u8], BackrubError>(BackrubError::SledKeyLengthError),
                    Ok,
                )?
                .try_into()?;

            // Check data
            let entry = InodeDbEntry::decrypt(&encrypted_data, &self.inode_enc_key)?;
            if key != Hash256::from(*entry.inode.keyed_hash(&self.inode_hash_key)?.as_bytes()) {
                return Err(BackrubError::SelfTestError.into());
            }
        }
        Ok(())
    }

    pub fn new(tree: sled::Tree, inode_enc_key: Key256, inode_hash_key: Key256) -> Result<InodeDb> {
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

    pub fn insert(&mut self, inode: Inode) -> Result<(RefCount, Hash256)> {
        let key = Hash256::from(*inode.keyed_hash(&self.inode_hash_key)?.as_bytes());
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

    pub fn remove(&mut self, key: &Hash256) -> Result<Option<(RefCount, Inode)>> {
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

    pub fn get_inode_db_entry(&self, key: &Hash256) -> Result<Option<(RefCount, Inode)>> {
        match self.tree.get(key)? {
            None => Ok(None),
            Some(encrypted_data) => {
                let entry = InodeDbEntry::decrypt(&encrypted_data, &self.inode_enc_key)?;
                Ok(Some((entry.ref_count, entry.inode)))
            }
        }
    }

    pub fn get_inode(&self, key: &Hash256) -> Result<Option<Inode>> {
        match self.get_inode_db_entry(key)? {
            None => Ok(None),
            Some((_ref_count, inode)) => Ok(Some(inode)),
        }
    }

    pub fn get_ref_count(&self, key: &Hash256) -> Result<Option<RefCount>> {
        match self.get_inode_db_entry(key)? {
            None => Ok(None),
            Some((ref_count, _inode)) => Ok(Some(ref_count)),
        }
    }

    pub fn get_mappings(&self) -> Result<BTreeMap<Hash256, (RefCount, Inode)>> {
        let mut result = BTreeMap::<Hash256, (RefCount, Inode)>::new();
        for data in self.tree.iter() {
            let (key, encrypted_data) = data?;
            if key.len() != HASH_SIZE {
                return Err(BackrubError::SledKeyLengthError.into());
            } else {
                let hash: Hash256 = key
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
