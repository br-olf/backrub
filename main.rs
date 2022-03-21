use clap::{Arg, Command, ValueHint};
use clap_complete::{generate, Shell};
use log::{error, info, warn};
//use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::exit;
use std::{error, fmt, fs, io};
use walkdir::WalkDir;

const APPNAME: &str = "dedup";
const TREE_EXTENSION: &str = "tree.json.zip";
const CONFIG_EXTENSION: &str = "ini";
const FOLLOW_LINKS: bool = true;

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

#[allow(unreachable_code)]
fn parse_config() {
    // TODO Parse config file
    todo!("Config and command line parsing is not implemented!");

    println!("Would read {}", default_conf_file().display());
    println!("Would save to {}", default_tree_file().display());

    let matches = build_cli().get_matches();

    match matches.subcommand() {
        Some(("print", s_print)) => {
            println!("Subcommand print was used: {:?}", s_print);
            todo!()
        }
        Some(("analyze", s_ana)) => {
            println!("Subcommand analyze was used: {:?}", s_ana);
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

#[allow(dead_code)]
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

#[allow(dead_code)]
fn test_stuff_2() {
    #[allow(unused_imports)]
    use std::time::{Duration, Instant};
    let mut v = Vec::<PathBuf>::new();
    let start = Instant::now();
    for file in WalkDir::new(".")
        .follow_links(false)
        .into_iter()
        .filter_map(|f| f.ok())
    {
        if file.metadata().unwrap().is_file() {
            v.push(file.path().to_path_buf());
        }
    }
    let duration = start.elapsed();
    println!("{:?}\n", v);
    println!("\nTook {:?}", duration);
}

#[allow(dead_code)]
fn test_stuff_5() {
    use rayon::prelude::*;
    use std::sync::mpsc::channel;

    let (sender, receiver) = channel();
    let v: Vec<u32> = (0..50).collect();
    v.into_par_iter().for_each_with(sender, |s, x| {
        s.send((x, *blake3::hash(&x.to_ne_bytes()).as_bytes()))
            .unwrap()
    });

    let res: Vec<_> = receiver.iter().collect();

    println!("{:?}", res);
}




#[derive(Debug)]
struct MultipleIoErrors {
    errors: Vec<(PathBuf, std::io::Error)>,
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

fn crawl_dir(path: &dyn AsRef<Path>) -> Result<Vec<PathBuf>, std::io::Error> {
    let dir_path = fs::canonicalize(path)?;
    if dir_path.is_file() {
        return Ok(vec![dir_path]);
    }
    if !dir_path.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "crawl_dir is expecting a file or directory",
        ));
    }

    let mut result = Vec::<PathBuf>::new();
    for file in WalkDir::new(dir_path)
        .follow_links(FOLLOW_LINKS)
        .into_iter()
        .filter_map(|f| f.ok())
    {
        if file.metadata().unwrap().is_file() {
            result.push(file.path().to_path_buf());
        }
    }
    Ok(result)
}

fn calculate_file_hashes(
    files: Vec<PathBuf>,
) -> Result<
    Vec<(PathBuf, Result<[u8; 32], std::io::Error>)>,
    std::sync::mpsc::SendError<(PathBuf, Result<[u8; 32], std::io::Error>)>,
> {
    use rayon::prelude::*;
    use std::sync::mpsc::channel;

    let (sender, receiver) = channel();

    files
        .into_par_iter()
        .try_for_each_with(sender, |s, file| match fs::read(file.clone()) {
            Ok(content) => s.send((file, Ok(*blake3::hash(&content).as_bytes()))),
            Err(e) => s.send((file, Err(e))),
        })?;

    Ok(receiver.iter().collect())
}

fn create_file_hash_tree(
    hash_results: Vec<(PathBuf, Result<[u8; 32], std::io::Error>)>,
) -> (BTreeMap<PathBuf, [u8; 32]>, Option<MultipleIoErrors>) {
    let mut data = BTreeMap::<PathBuf, [u8; 32]>::new();
    let mut errors = MultipleIoErrors {
        errors: Vec::<(PathBuf, std::io::Error)>::new(),
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

fn main() {
    env_logger::init();

    match crawl_dir(&".") {
        Err(e) => {
            error!("crawl_dir FAILED: {}", e);
        }
        Ok(files) => match calculate_file_hashes(files) {
            Err(e) => {
                error!("calculate_file_hashes FAILED:{}", e);
            }
            Ok(hash_vec) => {
                let (data, errors) = create_file_hash_tree(hash_vec);
                if let Some(e) = errors {
                    warn!(
                        "create_file_hash_tree returned {} errors: {}",
                        e.errors.len(),
                        e
                    );
                }
                info!("create_file_hash_tree returned {} elements", data.len());
            }
        },
    }

    println!();
    parse_config();
}
