use clap::{Arg, Command, ValueHint};
use clap_complete::{generate, Shell};
use rb_tree::RBTree;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
//use std::hash::{Hash, Hasher};
use std::error::Error;
use std::io;
use std::path::{Path, PathBuf};
use std::process::exit;

const APPNAME: &str = "dedup";

fn default_conf_file() -> PathBuf {
    match dirs::config_dir() {
        Some(dir) => {
            let mut path = dir;
            path.push(APPNAME);
            path.set_extension("ini");
            path
        }
        None => {
            let mut path = PathBuf::from(APPNAME);
            path.set_extension("ini");
            path
        }
    }
}
fn default_tree_file() -> PathBuf {
    match dirs::data_local_dir() {
        Some(dir) => {
            let mut path = dir;
            path.push(APPNAME);
            path.set_extension("tree");
            path
        }
        None => {
            let mut path = PathBuf::from(APPNAME);
            path.set_extension("tree");
            path
        }
    }
}

#[derive(Debug, Eq, Serialize, Deserialize)]
struct UniqeFileOld {
    hash: [u8; 32],
    locations: Vec<String>,
}
impl PartialEq for UniqeFileOld {
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash
    }
}
impl PartialOrd for UniqeFileOld {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.hash.partial_cmp(&other.hash)
    }
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
            let _ = build_cli().print_long_help();
            exit(1)
        }
    }
}

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

#[derive(Debug, Eq, Serialize, Deserialize)]
struct FilePath {
    location: PathBuf,
    hash: [u8; 32],
}
impl PartialEq for FilePath {
    fn eq(&self, other: &Self) -> bool {
        self.location == other.location
    }
}
impl PartialOrd for FilePath {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.location.partial_cmp(&other.location)
    }
}

#[derive(Debug, Eq, Serialize, Deserialize)]
struct FileHash {
    hash: [u8; 32],
    locations: Vec<PathBuf>,
}
impl PartialEq for FileHash {
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash
    }
}
impl PartialOrd for FileHash {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.hash.partial_cmp(&other.hash)
    }
}

/// Error type for unknown path in DedupData
#[derive(Debug, Clone)]
struct UnknownPath;

#[derive(Debug, Serialize, Deserialize)]
struct DedupData {
    hashtree: RBTree<FileHash>,
    pathtree: RBTree<FilePath>,
}

impl DedupData {
    fn update(&self, path: PathBuf, hash: [u8; 32]) {
        todo!()
    }
    fn delete_path(&self, path: PathBuf) -> Result<(), UnknownPath> {
        todo!()
    }
    fn delete_path_prefix(&self, path: PathBuf) -> Result<(), UnknownPath> {
        todo!()
    }
    fn find_duplicates_by_path(&self, path: PathBuf) -> Result<Vec<PathBuf>, UnknownPath> {
        todo!()
    }
    fn find_duplicates_by_hash(&self, hash: [u8; 32]) -> Vec<PathBuf> {
        todo!()
    }
    fn get_duplicates(&self) -> Vec<Vec<PathBuf>> {
        todo!()
    }
    fn new() -> DedupData {
        todo!()
    }
}

fn test_stuff() {
    let mut ptree = RBTree::<FilePath>::new();
    let mut htree = RBTree::<FileHash>::new();

    let fpath = PathBuf::from("/testfile.txt");
    let fcont = b"foobar and batz";

    let fhash = *blake3::hash(fcont).as_bytes();

    let fh = FileHash {
        hash: fhash,
        locations: vec![fpath.clone()],
    };
    let fp = FilePath {
        location: fpath.clone(),
        hash: fhash,
    };

    println!("{:?}", fpath.starts_with(fpath.clone()));
}

fn main() {
    //parse_config();

    test_stuff();
    //test_tree_and_balke3();
}
