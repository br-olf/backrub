use backrub::manager::*;
use std::io::stdin;
use std::path;
use std::sync::Arc;
use std::time::Instant;
use tempfile;

fn main() {
    use sanakirja::*;
    let dir = tempfile::tempdir().unwrap();
    println!("{:?}", dir);
    let path = dir.path().join("db");
    let env = Env::new(&path, 1 << 22, 20).unwrap();

    let now = Instant::now();
    let mut txn = Env::mut_txn_begin(&env).unwrap();
    let mut db = btree::create_db::<_, u64, u64>(&mut txn).unwrap();

    let N = 100_000u64;
    for i in 0..N {
        btree::put(&mut txn, &mut db, &i, &(i * i)).unwrap();
    }
    let root_db = 0;
    txn.set_root(root_db, db.db);
    txn.commit().unwrap();

    let elapsed = now.elapsed();
    println!("putting {} elements took {}ms", N, elapsed.as_millis());

    let now= Instant::now();
    let txn = Env::txn_begin(&env).unwrap();
    let db: btree::Db<u64, u64> = txn.root_db(root_db).unwrap();
    assert_eq!(
        btree::get(&txn, &db, &50_000, None).unwrap(),
        Some((&50_000, &(50_000 * 50_000)))
    );

    let elapsed = now.elapsed();
    println!("accessing middle element takes {}ms", elapsed.as_millis());
    let now = Instant::now();

    for entry in btree::iter(&txn, &db, None).unwrap() {
        let (k, v) = entry.unwrap();
        assert_eq!(*k * *k, *v)
    }

    let elapsed = now.elapsed();
    println!("full table scan took {}ms",elapsed.as_millis());


    let N = 1000u64;
    for i in 0..N {
        println!("{}",i);
    let mut txn = Env::mut_txn_begin(&env).unwrap();
    let mut db = btree::create_db::<_, u64, u64>(&mut txn).unwrap();
        btree::put(&mut txn, &mut db, &i, &(i * i)).unwrap();
    let root_db = 0;
    txn.set_root(root_db, db.db);
        println!("{:?}",txn.commit());
    }

    let elapsed = now.elapsed();
    println!("putting {} elements took {}ms", N, elapsed.as_millis());
}

fn main2() {
    let now = Instant::now();
    let res = index_dir(&path::Path::new("/home/olaf/Work")).unwrap();
    let elapsed_time = now.elapsed();

    let (dir, all, files) = res;
    println!(
        "OK\n\n\n{}\n{}\n",
        Arc::<TDirEntry>::strong_count(&dir),
        Arc::<TDirEntry>::weak_count(&dir)
    );
    println!("all entries: {}\nfiles: {}\n", all.len(), files.len());

    let mut max = 0;
    for e in all {
        let l = Arc::<TDirEntry>::strong_count(&e);
        if l > max {
            max = l;
        }
    }
    println!("{}\n\n", max);

    println!(
        "Running slow_function() took {} milliseconds.",
        elapsed_time.as_millis()
    );

    let testvec = vec![1, 2, 3, 4];
    let testarc: Arc<[i32]> = testvec.into();

    println!("testarc: {:?}", testarc);
}

// use clap::{Arg, Command, ValueHint};
// use clap_complete::{generate, Shell};
// use log::{error, info, warn};
// use std::io;
// use std::path::{Path, PathBuf};
// use std::process::exit;
// //use serde::{Deserialize, Serialize};
// use backrub::*;
//
// const APPNAME: &str = "dedup";
// const TREE_EXTENSION: &str = "tree.json.zip";
// const CONFIG_EXTENSION: &str = "ini";
//
// // static INI_TEMPLATE: &'static str = include_str!("dedup-template.ini");
//
// fn default_conf_file() -> PathBuf {
//     match dirs::config_dir() {
//         Some(dir) => {
//             let mut path = dir;
//             path.push(APPNAME);
//             path.set_extension(CONFIG_EXTENSION);
//             path
//         }
//         None => {
//             let mut path = PathBuf::from(APPNAME);
//             path.set_extension(CONFIG_EXTENSION);
//             path
//         }
//     }
// }
//
// fn default_tree_file() -> PathBuf {
//     match dirs::data_local_dir() {
//         Some(dir) => {
//             let mut path = dir;
//             path.push(APPNAME);
//             path.set_extension(TREE_EXTENSION);
//             path
//         }
//         None => {
//             let mut path = PathBuf::from(APPNAME);
//             path.set_extension(TREE_EXTENSION);
//             path
//         }
//     }
// }
//
// struct Config {
//     treeFilePath: PathBuf,
//     followSymlinks: bool,
// }
//
// impl Config {
//     fn new() -> Self {
//         Config {
//             treeFilePath: PathBuf::new(),
//             followSymlinks: true,
//         }
//     }
// }
//
// fn build_cli() -> Command<'static> {
//     Command::new(APPNAME)
//         .subcommand_required(true)
//         .subcommand(
//             Command::new("config")
//                 .long_flag("config")
//                 .short_flag('c')
//                 .about("Change or print the configuration"), //.arg(
//                                                              //    todo!("Todo");
//                                                              //),
//         )
//         .subcommand(
//             Command::new("completion")
//                 .long_flag("completion")
//                 .about("Print shell completions")
//                 .arg(
//                     Arg::new("shell")
//                         .possible_values(Shell::possible_values())
//                         .help("The shell for witch the completions are generated")
//                         .required(true),
//                 ),
//         )
//         .subcommand(
//             Command::new("print")
//                 .long_flag("print")
//                 .visible_long_flag_alias("ls")
//                 .short_flag('p')
//                 .about("Print duplicats in <tree> optionally restrict output to <path>")
//                 .arg(
//                     Arg::new("tree")
//                         .value_hint(ValueHint::FilePath)
//                         .help("The file where the tree is stored in")
//                         .required(true),
//                 )
//                 .arg(
//                     Arg::new("path")
//                         .help("The path results are filtered with")
//                         .value_hint(ValueHint::DirPath),
//                 ),
//         )
//         .subcommand(
//             Command::new("analyze")
//                 .long_flag("analyze")
//                 .short_flag('a')
//                 .about("Analyze <path> and update <tree>")
//                 .arg(
//                     Arg::new("tree")
//                         .help("The file where the tree is stored in")
//                         .value_hint(ValueHint::FilePath)
//                         .required(true),
//                 )
//                 .arg(
//                     Arg::new("path")
//                         .help("The path to analyze")
//                         .value_hint(ValueHint::DirPath)
//                         .required(true),
//                 ),
//         )
// }
//
// #[allow(unreachable_code)]
// fn parse_config() {
//     let config_file_path: PathBuf;
//     let config_file_path_env = std::env::var("DEDUP_CONFIG_PATH");
//     if config_file_path_env.is_err() {
//         config_file_path = default_conf_file();
//     } else {
//         config_file_path = Path::new(&config_file_path_env.unwrap()).to_path_buf();
//     }
//
//     let config = Config::new();
//     if config_file_path.exists() {
//         match std::fs::read_to_string(config_file_path.clone()) {
//             Ok(config_file) => {
//                 let mut ini = configparser::ini::Ini::new();
//                 ini.read(config_file.clone());
//             }
//             Err(e) => {
//                 error!(
//                     "Could not read cofig file {}:\n{}",
//                     config_file_path.display(),
//                     e
//                 );
//                 exit(1);
//             }
//         }
//     } else {
//         todo!("Load and save default config file")
//     }
//
//     // TODO Parse config file
//     todo!("Config and command line parsing is not implemented!");
//
//     println!("Would read {}", default_conf_file().display());
//     println!("Would save to {}", default_tree_file().display());
//
//     let matches = build_cli().get_matches();
//
//     match matches.subcommand() {
//         Some(("print", s_print)) => {
//             println!("Subcommand print was used: {:?}", s_print);
//             todo!()
//         }
//         Some(("analyze", s_ana)) => {
//             println!("Subcommand analyze was used: {:?}", s_ana);
//             todo!()
//         }
//         Some(("completion", s_comp)) => {
//             let shell = s_comp
//                 .value_of_t::<Shell>("shell")
//                 .unwrap_or_else(|e| e.exit());
//             generate(
//                 shell,
//                 &mut build_cli(),
//                 build_cli().get_name().to_string(),
//                 &mut io::stdout(),
//             );
//             exit(0)
//         }
//         _ => {
//             unreachable!("There should be no unimplemented subcommands")
//         }
//     }
// }
// use backrub::structs::*;
// use sled::{Error, IVec};
// use std::convert::TryInto;
// use once_cell::sync::OnceCell;
//
//
// static TEST: OnceCell<u64> = OnceCell::new();
//
// fn main() {
//     let config = sled::Config::new().temporary(true);
//     let db = config.open().unwrap();
//     let tree = db.open_tree(b"test").unwrap();
//     tree.insert(b"key", b"value");
//     tree.insert(b"k2",b"v2");
//     for t in tree.iter() {
//         match t {
//             Ok((k,v)) => {
//                 println!("{:?} => {:?}", k, v);
//             }
//             Err(e) => {
//                 println!("ERROR: {}", e);
//             }
//         }
//     }
//
//     TEST.set(64);
//     println!("{}", TEST.get().unwrap());
// }
//
// fn main2() {
//     env_logger::init();
//
//     let mut tree = DedupTree::new();
//     tree.update(".", true);
//
//     let json = tree.to_json();
//     let des_tree = DedupTree::from_json(&json).unwrap();
//
//     println!();
//     println!("json: {}", json);
//
//     println!();
//     println!("tree sizes: {}/{}", tree.len_unique(), tree.len_paths());
//     println!("found {} unique duplicates", tree.get_duplicates().len());
//
//     println!();
//     println!(
//         "des_tree sizes: {}/{}",
//         des_tree.len_unique(),
//         des_tree.len_paths()
//     );
//     println!(
//         "found {} unique duplicates",
//         des_tree.get_duplicates().len()
//     );
//
//     println!();
//     println!("tree == des_tree: {}", tree == des_tree);
//
//     tree.delete_dir(".").unwrap();
//     println!();
//     println!("deleted tree size: {}", tree.len_paths());
//
//     let bencoded: Vec<u8> = bincode::serialize(&des_tree).unwrap();
//     let bdecoded: DedupTree = bincode::deserialize(&bencoded[..]).unwrap();
//
//     println!();
//     println!("bindecoded == des_tree: {}", bdecoded == des_tree);
//
//     //   parse_config();
// }
