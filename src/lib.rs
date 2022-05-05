use std::collections::{BTreeMap, BTreeSet};
use std::{error, fmt, fs, io, path};
use walkdir::WalkDir;

#[derive(Debug, Default)]
pub struct MultipleIoErrors(Vec<(path::PathBuf, io::Error)>);
impl error::Error for MultipleIoErrors {}
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
            "crawl_dir expects a file or directory",
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
            Ok(content) => s
                .send((
                    file,
                    Ok(*convert_32u8_to_4u64(blake3::hash(&content).as_bytes())),
                ))
                .unwrap(),
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
            result._update_helper(file, hash);
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

    pub fn delete_file<P: Into<path::PathBuf>>(&mut self, file: P) -> bool {
        let file_buf = file.into();
        match self.file_tree.remove(&file_buf) {
            None => false,
            Some(old_hash) => {
                self._delete_from_hash_tree(old_hash, &file_buf.into_boxed_path());
                true
            }
        }
    }

    fn _delete_from_hash_tree(&mut self, hash: [u64; 4], file: &path::Path) {
        let ht_entry = self.hash_tree.get_mut(&hash).unwrap();
        if ht_entry.len() > 1 {
            ht_entry.retain(|x| *x != file);
        } else {
            self.hash_tree.remove(&hash);
        }
    }

    fn _update_helper(&mut self, new_file: path::PathBuf, new_hash: [u64; 4]) {
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
        let raw_path = path.into();
        let dir_path_expanded = fs::canonicalize(raw_path.clone());
        match dir_path_expanded {
            Ok(dir_path) => {
                match crawl_dir(dir_path.as_path(), follow_links) {
                    Ok(files) => {
                        let hash_vec = calculate_file_hashes(files.clone());
                        let mut errors = MultipleIoErrors::new();
                        /***************************************************************/
                        /* run through the newly generated hashes and update the trees */
                        /***************************************************************/
                        for (new_file, new_hash_result) in hash_vec {
                            match new_hash_result {
                                Ok(new_hash) => {
                                    self._update_helper(new_file, new_hash);
                                }
                                Err(e) => errors.add(new_file, e),
                            }
                        }
                        /**************************/
                        /* cleanup orphan entries */
                        /**************************/
                        let mut files_to_delete = Vec::<path::PathBuf>::new();
                        for (file, _) in self.file_tree.iter() {
                            if file.starts_with(dir_path.clone())
                                && files.binary_search(&file.clone()).is_err()
                            {
                                // file is in tree but was not found again
                                files_to_delete.push(file.to_path_buf());
                            }
                        }
                        for file in files_to_delete {
                            self.delete_file(file);
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
                    Err(e) => Some(MultipleIoErrors(vec![(dir_path, e)])),
                }
            }
            Err(e) => Some(MultipleIoErrors(vec![(raw_path, e)])),
        }
    }
}
