#[doc(inline)]
pub use std;

use std::collections::{BTreeMap, BTreeSet};
use std::{error as std_error, fmt, fs, io, path};
use walkdir::WalkDir;

/// Error and Result types
pub mod error;

/// Basic data structs
pub mod structs;

/// Traits in backrub
pub mod traits;

/// Backup manager and configuration
pub mod manager;

pub fn convert_32u8_to_4u64(input: &[u8; 32]) -> &[u64; 4] {
    unsafe { std::mem::transmute::<&[u8; 32], &[u64; 4]>(input) }
}

pub fn convert_4u64_to_32u8(input: &[u64; 4]) -> &[u8; 32] {
    unsafe { std::mem::transmute::<&[u64; 4], &[u8; 32]>(input) }
}

fn crawl_dir(path: &path::Path, follow_links: bool) -> Result<Vec<path::PathBuf>, io::Error> {
    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }
    if !path.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected a file or directory",
        ));
    }

    let mut result = Vec::<path::PathBuf>::new();
    for file in WalkDir::new(path)
        .follow_links(follow_links)
        .into_iter()
        .filter_map(|f| f.ok())
    {
        if file.metadata().unwrap().is_file() {
            result.push(fs::canonicalize(file.path()).unwrap().to_path_buf());
        }
    }
    result.sort();
    Ok(result)
}

fn calculate_file_hashes(
    files: Vec<path::PathBuf>,
) -> Vec<(path::PathBuf, Result<[u64; 4], io::Error>)> {
    use rayon::prelude::*;

    let (sender, receiver) = std::sync::mpsc::channel();
    files
        .into_par_iter()
        .for_each_with(sender, |s, file| match fs::read(file.clone()) {
            Ok(content) => {
                let mut hasher = blake3::Hasher::new_derive_key("foobar");
                hasher.update_rayon(&content);
                s.send((
                    file,
                    Ok(*convert_32u8_to_4u64(hasher.finalize().as_bytes())),
                ))
                .unwrap()
            }
            Err(e) => s.send((file, Err(e))).unwrap(),
        });

    receiver.iter().collect()
}

#[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct DedupTree {
    hash_tree: BTreeMap<[u64; 4], BTreeSet<path::PathBuf>>,
    file_tree: BTreeMap<path::PathBuf, [u64; 4]>,
}

impl DedupTree {
    pub fn to_json(&self) -> String {
        serde_json::to_string(&self.file_tree).unwrap()
    }

    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        let file_tree: BTreeMap<path::PathBuf, [u64; 4]> = serde_json::from_str(json)?;
        let mut result = DedupTree::new();
        for (file, hash) in file_tree {
            result._update(file, hash);
        }
        Ok(result)
    }

    pub fn len_unique(&self) -> usize {
        self.hash_tree.len()
    }

    pub fn len_paths(&self) -> usize {
        self.file_tree.len()
    }

    pub fn new() -> Self {
        DedupTree {
            hash_tree: BTreeMap::new(),
            file_tree: BTreeMap::new(),
        }
    }

    pub fn get_duplicates(&self) -> Vec<Vec<&path::Path>> {
        let mut result = Vec::<Vec<&path::Path>>::new();
        for (_, paths) in self.hash_tree.iter() {
            if paths.len() > 1 {
                let mut tmp = Vec::<&path::Path>::new();
                for entry in paths.iter() {
                    tmp.push(entry.as_path());
                }
                result.push(tmp);
            }
        }
        result
    }

    pub fn delete_file<P: Into<path::PathBuf>>(&mut self, file: P) -> Result<bool, io::Error> {
        let file_buf = fs::canonicalize(file.into())?;
        if !file_buf.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "expected a file",
            ));
        }
        Ok(self._delete_file(file_buf))
    }

    fn _delete_file(&mut self, file_buf: path::PathBuf) -> bool {
        match self.file_tree.remove(&file_buf) {
            None => false,
            Some(old_hash) => {
                self._delete_from_hash_tree(old_hash, &file_buf.into_boxed_path());
                true
            }
        }
    }

    pub fn delete_dir<P: Into<path::PathBuf>>(&mut self, dir: P) -> Result<bool, io::Error> {
        let dir_buf = fs::canonicalize(dir.into())?;
        if !dir_buf.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "expected a directory",
            ));
        }

        let mut deleted_something = false;
        let mut files_to_delete = Vec::<path::PathBuf>::new();
        for (file, _) in self.file_tree.iter() {
            if file.starts_with(dir_buf.clone()) {
                deleted_something = true;
                files_to_delete.push(file.to_path_buf());
            }
        }
        for file in files_to_delete {
            self._delete_file(file);
        }
        Ok(deleted_something)
    }

    fn _delete_from_hash_tree(&mut self, hash: [u64; 4], file: &path::Path) {
        let ht_entry = self.hash_tree.get_mut(&hash).unwrap();
        if ht_entry.len() > 1 {
            ht_entry.retain(|x| *x != file);
        } else {
            self.hash_tree.remove(&hash);
        }
    }

    fn _update(&mut self, new_file: path::PathBuf, new_hash: [u64; 4]) {
        // replace file_tree entry and lookup if the file was already registered
        if let Some(old_hash) = self.file_tree.insert(new_file.clone(), new_hash) {
            self._delete_from_hash_tree(old_hash, &new_file.clone().into_boxed_path());
        }
        // insert the new file into hash_tree
        match self.hash_tree.get_mut(&new_hash) {
            None => {
                self.hash_tree.insert(new_hash, BTreeSet::from([new_file]));
            }
            Some(entry) => {
                entry.insert(new_file);
            }
        }
    }

    pub fn update<P: Into<path::PathBuf>>(
        &mut self,
        path: P,
        follow_links: bool,
    ) -> Option<MultipleIoErrors> {
        let path_buf = path.into();
        let canonicalized_path = fs::canonicalize(path_buf.clone());
        match canonicalized_path {
            Ok(sanitized_path) => {
                match crawl_dir(sanitized_path.as_path(), follow_links) {
                    Ok(files) => {
                        let hash_vec = calculate_file_hashes(files.clone());
                        let mut errors = MultipleIoErrors::new();
                        /***************************************************************/
                        /* run through the newly generated hashes and update the trees */
                        /***************************************************************/
                        for (new_file, new_hash_result) in hash_vec {
                            match new_hash_result {
                                Ok(new_hash) => {
                                    self._update(new_file, new_hash);
                                }
                                Err(e) => errors.add(new_file, e),
                            }
                        }
                        /**************************/
                        /* cleanup orphan entries */
                        /**************************/
                        let mut files_to_delete = Vec::<path::PathBuf>::new();
                        for (file, _) in self.file_tree.iter() {
                            if file.starts_with(sanitized_path.clone())
                                && files.binary_search(&file.clone()).is_err()
                            {
                                // file is in tree but was not found again
                                files_to_delete.push(file.to_path_buf());
                            }
                        }
                        for file in files_to_delete {
                            self._delete_file(file);
                        }
                        /******************/
                        /* error handling */
                        /******************/
                        if errors.is_empty() {
                            None
                        } else {
                            Some(errors)
                        }
                    }
                    Err(e) => Some(MultipleIoErrors(vec![(sanitized_path, e)])),
                }
            }
            Err(e) => Some(MultipleIoErrors(vec![(path_buf, e)])),
        }
    }
}

#[derive(Debug, Default)]
pub struct MultipleIoErrors(Vec<(path::PathBuf, io::Error)>);

impl std_error::Error for MultipleIoErrors {}

impl fmt::Display for MultipleIoErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (p, e) in self.iter() {
            write!(f, "\n{}: {}", p.display(), e)?
        }
        Ok(())
    }
}

impl MultipleIoErrors {
    pub fn iter(&self) -> std::slice::Iter<(path::PathBuf, io::Error)> {
        self.0.iter()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn new() -> Self {
        MultipleIoErrors(Vec::new())
    }

    pub fn add<P: Into<path::PathBuf>>(&mut self, path: P, err: io::Error) {
        self.0.push((path.into(), err))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_MultipleIOErrors() {
        let instance = MultipleIoErrors::new();
        assert!(instance.is_empty());
    }
    #[test]
    fn filled_MultipleIOErrors() {
        let mut instance = MultipleIoErrors::new();
        instance.add(
            "/a/path",
            io::Error::new(io::ErrorKind::InvalidInput, "a error message"),
        );
        instance.add(
            path::PathBuf::from("/another/path"),
            io::Error::new(io::ErrorKind::InvalidInput, "another error message"),
        );
        assert_eq!(instance.len(), 2);
    }
    #[test]
    fn find_duplicates() {
        test_with_dir(|dir: path::PathBuf| {
            let mut tree = DedupTree::new();
            tree.update(dir, false);
            assert_eq!(tree.len_paths(), 3);
            assert_eq!(tree.len_unique(), 2);
        });
    }

    fn test_with_dir<T>(test: T) -> ()
    where
        T: Fn(path::PathBuf) + std::panic::RefUnwindSafe,
    {
        // Setup temporary directory
        use rand::distributions::Alphanumeric;
        use rand::prelude::*;
        use std::io::Write;

        let mut rng = StdRng::from_entropy();
        let dirname: String = rng
            .sample_iter(&Alphanumeric)
            .take(30)
            .map(char::from)
            .collect();

        let mut dir = std::env::temp_dir();
        dir.push(dirname);
        fs::create_dir(dir.clone()).expect("Test was not possible due to an unrelated io::Error");

        // put some files into this directory
        let mut file1_path = dir.clone();
        file1_path.push("foo.txt");
        let mut file1 = fs::File::create(file1_path)
            .expect("Test was not possible due to an unrelated io::Error");
        file1
            .write_all(b"Hello, world!")
            .expect("Test was not possible due to an unrelated io::Error");
        file1
            .sync_all()
            .expect("Test was not possible due to an unrelated io::Error");

        let mut file2_path = dir.clone();
        file2_path.push("bar.txt");
        let mut file2 = fs::File::create(file2_path)
            .expect("Test was not possible due to an unrelated io::Error");
        file2
            .write_all(b"Hello, world!")
            .expect("Test was not possible due to an unrelated io::Error");
        file2
            .sync_all()
            .expect("Test was not possible due to an unrelated io::Error");

        let mut file3_path = dir.clone();
        file3_path.push("baz");
        let mut file3 = fs::File::create(file3_path)
            .expect("Test was not possible due to an unrelated io::Error");
        file3
            .write_all(&[0xab, 0xcd, 0x12, 0x43])
            .expect("Test was not possible due to an unrelated io::Error");
        file3
            .sync_all()
            .expect("Test was not possible due to an unrelated io::Error");

        // run the test with this environment
        let result = std::panic::catch_unwind(|| test(dir.clone()));

        // cleanup temporary directory
        fs::remove_dir_all(dir).expect("Test was not possible due to an unrelated io::Error");

        // test the result
        assert!(result.is_ok())
    }
}
