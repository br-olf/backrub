use clap::{Arg, Command, ValueHint};
use clap_complete::{generate, Shell};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::process::exit;
use std::{error, fmt, fs, io};

const APPNAME: &str = "dedup";
const TREE_EXTENSION: &str = "tree.json.zip";
const CONFIG_EXTENSION: &str = "ini";

fn default_conf_file() -> PathBuf {
    match dirs::config_dir() {
        Some(dir) => {
            let mut path = dir;
            path.push(APPNAME);
            path.set_extension(CONFIG_EXTENSION);
            path
        }
        None => {
            let mut path = PathBuf::from(APPNAME);
            path.set_extension(CONFIG_EXTENSION);
            path
        }
    }
}
fn default_tree_file() -> PathBuf {
    match dirs::data_local_dir() {
        Some(dir) => {
            let mut path = dir;
            path.push(APPNAME);
            path.set_extension(TREE_EXTENSION);
            path
        }
        None => {
            let mut path = PathBuf::from(APPNAME);
            path.set_extension(TREE_EXTENSION);
            path
        }
    }
}

const FOLLOW_LINKS: bool = true;

fn traverse_tree(path: PathBuf) -> Result<Vec<PathBuf>, Box<dyn error::Error>> {
    todo!()
}

fn build_cli() -> Command<'static> {
    Command::new(APPNAME)
        .subcommand_required(true)
        .subcommand(
            Command::new("completion")
                .long_flag("completion")
                .short_flag('c')
                .about("Print shell completions")
                .arg(
                    Arg::new("shell")
                        .possible_values(Shell::possible_values())
                        .help("The shell for witch the completions are generated")
                        .required(true),
                ),
        )
        .subcommand(
            Command::new("print")
                .long_flag("print")
                .visible_long_flag_alias("ls")
                .short_flag('p')
                .about("Print duplicats in <tree> optionally restrict output to <path>")
                .arg(
                    Arg::new("tree")
                        .value_hint(ValueHint::FilePath)
                        .help("The file where the tree is stored in")
                        .required(true),
                )
                .arg(
                    Arg::new("path")
                        .help("The path results are filtered with")
                        .value_hint(ValueHint::DirPath),
                ),
        )
        .subcommand(
            Command::new("analyze")
                .long_flag("analyze")
                .short_flag('a')
                .about("Analyze <path> and update <tree>")
                .arg(
                    Arg::new("tree")
                        .help("The file where the tree is stored in")
                        .value_hint(ValueHint::FilePath)
                        .required(true),
                )
                .arg(
                    Arg::new("path")
                        .help("The path to analyze")
                        .value_hint(ValueHint::DirPath)
                        .required(true),
                ),
        )
}

fn parse_config() {
    // TODO Parse config file

    let matches = build_cli().get_matches();

    match matches.subcommand() {
        Some(("print", s_print)) => {
            println!("Subcommand print was used");
            todo!()
        }
        Some(("analyze", s_ana)) => {
            println!("Subcommand analyze was used");
            todo!()
        }
        Some(("completion", s_comp)) => {
            let shell = s_comp
                .value_of_t::<Shell>("shell")
                .unwrap_or_else(|e| e.exit());
            generate(
                shell,
                &mut build_cli(),
                build_cli().get_name().to_string(),
                &mut io::stdout(),
            );
            exit(0)
        }
        _ => {
            unreachable!("There should be no unimplemented subcommands")
        }
    }
}

/*
fn test_tree_and_balke3() {
    let mut tree = RBTree::<UniqeFileOld>::new();

    // Hash an input all at once.
    let hash1 = blake3::hash(b"foobarbaz");

    // Hash an input incrementally.
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"foo");
    hasher.update(b"bar");
    hasher.update(b"baz");
    let hash2 = hasher.finalize();
    assert_eq!(hash1, hash2);

    // Extended output. OutputReader also implements Read and Seek.
    let mut output = [0; 1000];
    let mut output_reader = hasher.finalize_xof();
    output_reader.fill(&mut output);
    assert_eq!(&output[..32], hash1.as_bytes());

    // Print a hash as hex.
    println!("{}", hash2);

    let test = UniqeFileOld {
        hash: *hash1.as_bytes(),
        locations: vec!["foo".to_string()],
    };

    println!("{:?}", test);

    tree.insert(test);

    tree.insert(UniqeFileOld {
        hash: *blake3::hash(b"bar").as_bytes(),
        locations: vec!["bar".to_string()],
    });
    tree.insert(UniqeFileOld {
        hash: *blake3::hash(b"Bier").as_bytes(),
        locations: vec!["Bier".to_string()],
    });
    println!("\n{:?}", tree);

    println!(
        "\n{}",
        tree.contains(&UniqeFileOld {
            hash: *hash1.as_bytes(),
            locations: vec!("lolo".to_string())
        })
    );

    println!("\n\n");

    let serialized = serde_json::to_string(&tree).unwrap();
    println!("serialized = {}", serialized);

    let deserialized: RBTree<UniqeFileOld> = serde_json::from_str(&serialized).unwrap();
    println!("deserialized = {:?}", deserialized);
    assert!(tree.is_subset(&deserialized));
    assert!(tree.is_superset(&deserialized));
}
 */

/// Error type for invalid path in DedupData
#[derive(Debug, Clone)]
struct InvalidPath;
impl error::Error for InvalidPath {}
impl fmt::Display for InvalidPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct DedupData {
    hashtree: BTreeMap<[u8; 32], BTreeSet<PathBuf>>,
    pathtree: BTreeMap<PathBuf, [u8; 32]>,
}

fn filepath_to_absolute(path: PathBuf) -> Result<PathBuf, Box<dyn error::Error>> {
    if !path.is_file() {
        return Err(Box::new(InvalidPath));
    }
    Ok(std::fs::canonicalize(path)?)
}

impl DedupData {
    fn update(mut self, path: PathBuf, hash: [u8; 32]) -> Result<(), Box<dyn error::Error>> {
        let location = filepath_to_absolute(path)?;

        match self.pathtree.get_mut(&location) {
            //File is kown
            Some(mut_pt_h) => {
                // File is known but hash has changed
                if *mut_pt_h != hash {
                    // Update hashtree
                    let mut_ht_locations = self.hashtree.get_mut(&mut_pt_h.clone()).unwrap();

                    // File is the only one with that hash
                    if mut_ht_locations.len() == 1 {
                        self.hashtree.remove(&mut_pt_h.clone());
                    }
                    // File is a duplicate
                    else {
                        mut_ht_locations.remove(&location);
                    }

                    // Update pathree hash
                    *mut_pt_h = hash;
                }
                // File is known and not changed
                else {
                }
            }
            // File is not known
            None => {
                match self.hashtree.get_mut(&hash) {
                    // File is not known but a duplicate
                    Some(mut_ht_locations) => {
                        // Add path to hashtree locations
                        mut_ht_locations.insert(location.clone());
                        // Add file to pathtree
                        self.pathtree.insert(location, hash);
                    }
                    // File is not known and no duplicate
                    None => {
                        self.pathtree.insert(location.clone(), hash);
                        self.hashtree.insert(hash, BTreeSet::from([location]));
                    }
                }
            }
        }
        Ok(())
    }
    fn delete_path(&mut self, path: PathBuf) -> Result<bool, Box<dyn error::Error>> {
        let location = filepath_to_absolute(path)?;
        match self.pathtree.remove(&location) {
            Some(hash) => {
                let mut_ht_locations = self.hashtree.get_mut(&hash).unwrap();
                // File is the only one with that hash
                if mut_ht_locations.len() == 1 {
                    self.hashtree.remove(&hash);
                }
                // File is a duplicate
                else {
                    mut_ht_locations.remove(&location);
                }
                Ok(true)
            }
            None => Ok(false),
        }
    }
    fn delete_path_prefix(
        &mut self,
        path: PathBuf,
    ) -> Result<BTreeSet<PathBuf>, Box<dyn error::Error>> {
        let path_prefix = std::fs::canonicalize(path)?;
        let mut deleted = BTreeSet::<PathBuf>::new();
        for (location, _hash) in &self.pathtree {
            if location.starts_with(path_prefix.clone()) {
                deleted.insert(location.clone());
            }
        }
        for location in &deleted {
            // TODO: Here is space for optimisation
            self.delete_path(location.clone()).unwrap();
        }
        Ok(deleted)
    }

    fn find_duplicates_by_path(
        &self,
        path: PathBuf,
    ) -> Result<BTreeSet<PathBuf>, Box<dyn std::error::Error>> {
        todo!()
    }
    fn find_duplicates_by_path_prefix(
        &self,
        path_prefix: PathBuf,
    ) -> Result<BTreeMap<PathBuf, BTreeSet<PathBuf>>, Box<dyn std::error::Error>> {
        todo!()
    }
    fn get_duplicates(&self) -> Vec<BTreeSet<PathBuf>> {
        let mut result = Vec::<BTreeSet<PathBuf>>::new();
        for (_hash, locations) in &self.hashtree {
            if locations.len() > 1 {
                result.push(locations.clone());
            }
        }
        return result;
    }
    fn new() -> DedupData {
        DedupData {
            hashtree: BTreeMap::<[u8; 32], BTreeSet<PathBuf>>::new(),
            pathtree: BTreeMap::<PathBuf, [u8; 32]>::new(),
        }
    }
}

fn test_stuff_1() {
    println!("default conf file location = {:?}", default_conf_file());
    println!("default tree file location = {:?}", default_tree_file());
}

fn test_stuff_3() {
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let vals = vec![
            String::from("hi"),
            String::from("from"),
            String::from("the"),
            String::from("thread"),
        ];

        for val in vals {
            tx.send(val).unwrap();
            thread::sleep(Duration::from_secs(1));
        }
    });

    for received in rx {
        println!("Got: {}", received);
    }
}
/*
fn test_stuff_4() {

     // cargo.toml:
     // [dependencies]
     // crossbeam-channel = "0.5.1"

    use crossbeam_channel::{unbounded, Receiver, Sender};
    use std::thread::{sleep, spawn};
    use std::time::Duration;

    fn consumer(thread: i32, request: Sender<bool>, response: Receiver<u64>) {
        let mut receive_counter = 3;
        loop {
            request.send(true).unwrap();
            let r = response.recv().unwrap();
            println!("Thread {} received {}", thread, r);
            receive_counter -= 1;
            if receive_counter == 0 {
                println!("Thread {} is done!", thread);
                break;
            } else {
                sleep(Duration::from_secs(r))
            }
        }
    }

    fn producer(mut vec_u64: Vec<u64>, request: Receiver<bool>, response: Sender<u64>) {
        loop {
            match request.try_recv() {
                Ok(_) => {
                    let send_val = vec_u64.swap_remove(0);
                    response.send(send_val).unwrap();
                    if vec_u64.len() == 0 {
                        println!("Finishing producing");
                        break;
                    }
                }
                _ => {}
            }
        }
    }

    let (tx1, rx1) = unbounded();
    let (tx2, rx2) = unbounded();
    for i in 0..3 {
        let tx1 = tx1.clone();
        let rx2 = rx2.clone();
        spawn(move || consumer(i, tx1, rx2));
    }
    let vec_int: Vec<u64> = vec![3, 1, 2, 2, 1, 3, 4, 4, 2];
    spawn(move || producer(vec_int, rx1, tx2));
    loop {}
}
 */



fn test_stuff_2() {
    use walkdir::WalkDir;
    use std::time::{Duration, Instant};
    let mut v = Vec::<PathBuf>::new();
    let start = Instant::now();
    for file in WalkDir::new("/home/").follow_links(false).into_iter().filter_map(|f| f.ok()) {
        if file.metadata().unwrap().is_file() {
            v.push(file.path().to_path_buf());
        }
    }
    let duration = start.elapsed();
    println!("{:?}\n", v);
    println!("\nTook {:?}", duration);
}

fn test_stuff_5() {
    use rayon::prelude::*;
    use std::sync::mpsc::channel;

    let (sender, receiver) = channel();
    let v: Vec<u32> = (0..50).collect();
    v.into_par_iter().for_each_with(sender, |s, x| s.send((x, *blake3::hash(&x.to_ne_bytes()).as_bytes())).unwrap());

    let mut res: Vec<_> = receiver.iter().collect();

    println!("{:?}", res);
}

fn main() {
    test_stuff_5();
//    parse_config();

    //test_tree_and_balke3();
}
