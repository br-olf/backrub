use serde;
use serde_json;
use std::collections::BTreeMap;
use std::{error, fmt, fs, io, path};
use walkdir::WalkDir;


#[derive(Debug)]
pub struct MultipleIoErrors {
    errors: Vec<(path::PathBuf, io::Error)>,
}
impl error::Error for MultipleIoErrors {}
impl fmt::Display for MultipleIoErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (p, e) in &self.errors {
            write!(f, "\n{}: {}", p.display(), e)?
        }
        Ok(())
    }
}
impl MultipleIoErrors {
    pub fn len(&self) -> usize {
        self.errors.len()
    }
    pub fn make_new(errors: Vec<(path::PathBuf, io::Error)>) -> Self {
        MultipleIoErrors { errors: errors }
    }
    pub fn new() -> Self {
        MultipleIoErrors { errors: Vec::new() }
    }
    pub fn add(&mut self, path: path::PathBuf, err: io::Error) {
        self.errors.push((path, err))
    }
}

pub fn convert_32u8_to_4u64(input: &[u8; 32]) -> &[u64; 4] {
    unsafe {std::mem::transmute::<&[u8; 32], &[u64; 4]>(input)}
}

pub fn convert_4u64_to_32u8(input: &[u64; 4]) -> &[u8; 32] {
    unsafe {std::mem::transmute::<&[u64; 4], &[u8; 32]>(input)}
}

pub fn crawl_dir<P: Into<path::PathBuf>>(
    path: P,
    follow_links: bool,
) -> Result<Vec<path::PathBuf>, io::Error> {
    let dir_path = fs::canonicalize(path.into())?;
    if dir_path.is_file() {
        return Ok(vec![dir_path]);
    }
    if !dir_path.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "crawl_dir expects a file or directory",
        ));
    }

    let mut result = Vec::<path::PathBuf>::new();
    for file in WalkDir::new(dir_path)
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

pub fn calculate_file_hashes(
    files: Vec<path::PathBuf>,
) -> Vec<(path::PathBuf, Result<[u8; 32], io::Error>)> {
    use rayon::prelude::*;

    let (sender, receiver) = std::sync::mpsc::channel();
    files
        .into_par_iter()
        .for_each_with(sender, |s, file| match fs::read(file.clone()) {
            Ok(content) => s
                .send((file, Ok(*blake3::hash(&content).as_bytes())))
                .unwrap(),
            Err(e) => s.send((file, Err(e))).unwrap(),
        });

    receiver.iter().collect()
}

pub fn create_file_hash_tree(
    hash_results: Vec<(path::PathBuf, Result<[u8; 32], io::Error>)>,
) -> (
    BTreeMap<path::PathBuf, [u8; 32]>,
    BTreeMap<[u8; 32], Vec<path::PathBuf>>,
    Option<MultipleIoErrors>,
) {
    let mut file_tree = BTreeMap::<path::PathBuf, [u8; 32]>::new();
    let mut hash_tree = BTreeMap::<[u8; 32], Vec<path::PathBuf>>::new();
    let mut errors = MultipleIoErrors {
        errors: Vec::<(path::PathBuf, io::Error)>::new(),
    };
    let mut has_errors: bool = false;

    for (path, res) in hash_results {
        match res {
            Ok(hash) => {
                file_tree.insert(path.clone(), hash.clone());
                match hash_tree.get_mut(&hash) {
                    Some(entry) => entry.push(path),
                    None => {
                        hash_tree.insert(hash, vec![path]);
                    }
                }
            }
            Err(err) => {
                errors.errors.push((path, err));
                has_errors = true;
            }
        }
    }
    if has_errors {
        (file_tree, hash_tree, Some(errors))
    } else {
        (file_tree, hash_tree, None)
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct DedupTree {
    hash_tree: BTreeMap<[u8; 32], Vec<path::PathBuf>>,
    file_tree: BTreeMap<path::PathBuf, [u8; 32]>,
}

impl DedupTree {
    pub fn to_json(&self) -> String {
        serde_json::to_string(&self).unwrap()
    }

    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    pub fn len_unique(&self) -> usize {
        return self.hash_tree.len();
    }
    pub fn len_paths(&self) -> usize {
        return self.file_tree.len();
    }

    pub fn new() -> Self {
        DedupTree {
            hash_tree: BTreeMap::new(),
            file_tree: BTreeMap::new(),
        }
    }

    pub fn delete_file<P: Into<path::PathBuf>>(&mut self, file: P) -> Option<[u8; 32]> {
        let file_buf = file.into();
        match self.file_tree.remove(&file_buf) {
            None => {
                return None;
            }
            Some(old_hash) => {
                self._delete_from_hash_tree(old_hash, &file_buf.into_boxed_path());
                return Some(old_hash);
            }
        }
    }

    fn _delete_from_hash_tree(&mut self, hash: [u8; 32], file: &path::Path) {
        let ht_entry = self.hash_tree.get_mut(&hash).unwrap();
        if ht_entry.len() > 1 {
            ht_entry.retain(|x| *x != file);
        } else {
            self.hash_tree.remove(&hash);
        }
    }
    pub fn update<P: Into<path::PathBuf>>(
        &mut self,
        path: P,
        follow_links: bool,
    ) -> Option<MultipleIoErrors> {
        let raw_path = path.into();
        let dir_path_expanded = fs::canonicalize(raw_path.clone());
        match dir_path_expanded {
            Ok(dir_path) => {
                match crawl_dir(dir_path.clone(), follow_links) {
                    Ok(files) => {
                        let hash_vec = calculate_file_hashes(files.clone());
                        let mut errors = MultipleIoErrors::new();
                        /***************************************************************/
                        /* run through the newly generated hashes and update the trees */
                        /***************************************************************/
                        for (new_file, new_hash_result) in hash_vec {
                            match new_hash_result {
                                Ok(new_hash) => {
                                    // replace file_tree entry and lookup if the file was already registered
                                    if let Some(old_hash) =
                                        self.file_tree.insert(new_file.clone(), new_hash)
                                    {
                                        self._delete_from_hash_tree(
                                            old_hash,
                                            &new_file.clone().into_boxed_path(),
                                        );
                                    }
                                    // insert the new file into hash_tree
                                    match self.hash_tree.get_mut(&new_hash) {
                                        None => {
                                            self.hash_tree.insert(new_hash, vec![new_file]);
                                        }
                                        Some(entry) => {
                                            entry.push(new_file);
                                        }
                                    }
                                }
                                Err(e) => errors.add(new_file, e),
                            }
                        }
                        /**************************/
                        /* cleanup orphan entries */
                        /**************************/
                        let mut files_to_delete = Vec::<path::PathBuf>::new();
                        for (file, _) in self.file_tree.iter() {
                            if file.starts_with(dir_path.clone()) {
                                if let Err(_) = files.binary_search(&file.clone()) {
                                    // file is in tree but was not found again
                                    files_to_delete.push(file.to_path_buf());
                                }
                            }
                        }
                        for file in files_to_delete {
                            self.delete_file(file);
                        }
                        /******************/
                        /* error handling */
                        /******************/
                        if errors.len() > 0 {
                            return Some(errors);
                        } else {
                            return None;
                        }
                    }
                    Err(e) => {
                        return Some(MultipleIoErrors::make_new(vec![(dir_path, e)]));
                    }
                }
            }
            Err(e) => {
                return Some(MultipleIoErrors::make_new(vec![(raw_path, e)]));
            }
        }
    }
}
