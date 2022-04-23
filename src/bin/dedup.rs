use std::path::{PathBuf};
use clap::{Arg, Command, ValueHint};
use clap_complete::{generate, Shell};
use log::{error, info, warn};
use std::process::exit;
use std::io;
//use serde::{Deserialize, Serialize};
use dedup::*;

const APPNAME: &str = "dedup";
const TREE_EXTENSION: &str = "tree.json.zip";
const CONFIG_EXTENSION: &str = "ini";

static INI_TEMPLATE: &'static str = include_str!("dedup-template.ini");

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
    let config_file: PathBuf;
    let config_file_env = std::env::var("DEDUP_CONFIG_PATH");
    if config_file_env.is_err(){
        config_file = default_conf_file();
    }
    else {
        config_file = std::path::Path::new(&config_file_env.unwrap()).to_path_buf();
    }


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
fn main() {
    env_logger::init();

    match crawl_dir(&".") {
        Err(e) => {
            error!("crawl_dir FAILED: {}", e);
        }
        Ok(files) => {
            let hash_vec = calculate_file_hashes(files);
            let (data, errors) = create_file_hash_tree(hash_vec);
            if let Some(e) = errors {
                warn!("create_file_hash_tree returned {} errors: {}", e.len(), e);
            }
            info!("create_file_hash_tree returned {} elements", data.len());
        }
    }

    println!();
 //   parse_config();
}