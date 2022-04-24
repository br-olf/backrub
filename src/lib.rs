use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::{error, fmt, fs, io};
use walkdir::WalkDir;

#[derive(Debug)]
pub struct MultipleIoErrors {
    errors: Vec<(PathBuf, io::Error)>,
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
}

pub fn crawl_dir(path: &dyn AsRef<Path>, follow_links: bool) -> Result<Vec<PathBuf>, io::Error> {
    let dir_path = fs::canonicalize(path)?;
    if dir_path.is_file() {
        return Ok(vec![dir_path]);
    }
    if !dir_path.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "crawl_dir expects a file or directory",
        ));
    }

    let mut result = Vec::<PathBuf>::new();
    for file in WalkDir::new(dir_path)
        .follow_links(follow_links)
        .into_iter()
        .filter_map(|f| f.ok())
    {
        if file.metadata().unwrap().is_file() {
            result.push(file.path().to_path_buf());
        }
    }
    Ok(result)
}

pub fn calculate_file_hashes(files: Vec<PathBuf>) -> Vec<(PathBuf, Result<[u8; 32], io::Error>)> {
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
    hash_results: Vec<(PathBuf, Result<[u8; 32], io::Error>)>,
) -> (BTreeMap<PathBuf, [u8; 32]>, Option<MultipleIoErrors>) {
    let mut data = BTreeMap::<PathBuf, [u8; 32]>::new();
    let mut errors = MultipleIoErrors {
        errors: Vec::<(PathBuf, io::Error)>::new(),
    };
    let mut has_errors: bool = false;

    for (path, res) in hash_results {
        match res {
            Ok(hash) => {
                data.insert(path, hash);
            }
            Err(err) => {
                errors.errors.push((path, err));
                has_errors = true;
            }
        }
    }
    if has_errors {
        (data, Some(errors))
    } else {
        (data, None)
    }
}
