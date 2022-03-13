use clap::{AppSettings, Arg, Command, ValueHint};
use clap_complete::{generate, Generator, Shell};
use rb_tree::RBTree;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::io;

#[derive(Debug, Clone, Eq, Serialize, Deserialize)]
struct UniqueFile {
    hash: [u8; 32],
    locations: Vec<String>,
}
impl PartialEq for UniqueFile {
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash
    }
}
impl PartialOrd for UniqueFile {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.hash.partial_cmp(&other.hash)
    }
}
impl Hash for UniqueFile {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.hash.hash(state);
    }
}
use clap::Parser;

/*
/// Search for duplicate files and store them in a binary tree.
#[derive(Parser)]
struct Cli {
    /// The file where the tree is stored in
    #[clap(parse(from_os_str))]
    tree: std::path::PathBuf,
    /// The path to the file or directory to find duplicates in
    #[clap(parse(from_os_str))]
    path: std::path::PathBuf,
}
*/

fn build_cli() -> Command<'static> {
    Command::new("example")
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
                        .value_hint(ValueHint::DirPath)
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
fn print_completions<G: Generator>(gen: G, cmd: &mut Command) {
    generate(gen, cmd, cmd.get_name().to_string(), &mut io::stdout());
}

fn main() {
    let matches = build_cli().get_matches();

    println!("{:?}", matches);

    match matches.subcommand() {
        Some(("print", s_print)) => { println!("Subcommand print was used") },
        Some(("analyze", s_ana)) => { println!("Subcommand analyze was used") },
        Some(("completion", s_comp)) => {
            let shell = s_comp.value_of_t::<Shell>("shell").unwrap_or_else(|e| e.exit());
            let mut cmd = build_cli();
            print_completions(shell, &mut cmd)
        },
        Some((&_, _)) => {},
        None => {},
    }

    /*
    if let Ok(generator) = matches.value_of_t::<Shell>("generator") {
        let mut cmd = build_cli();
        eprintln!("Generating completion file for {}...", generator);
        print_completions(generator, &mut cmd);

    }
     */
    return;

    let mut tree = RBTree::<UniqueFile>::new();

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

    let test = UniqueFile {
        hash: *hash1.as_bytes(),
        locations: vec!["foo".to_string()],
    };

    println!("{:?}", test);

    tree.insert(test.clone());

    tree.insert(UniqueFile {
        hash: *blake3::hash(b"bar").as_bytes(),
        locations: vec!["bar".to_string()],
    });
    tree.insert(UniqueFile {
        hash: *blake3::hash(b"Bier").as_bytes(),
        locations: vec!["Bier".to_string()],
    });
    println!("\n{:?}", tree);

    println!(
        "\n{}",
        tree.contains(&UniqueFile {
            hash: *hash1.as_bytes(),
            locations: vec!("lolo".to_string())
        })
    );

    println!("\n\n");

    let serialized = serde_json::to_string(&test).unwrap();
    println!("serialized = {}", serialized);

    let deserialized: UniqueFile = serde_json::from_str(&serialized).unwrap();
    println!("deserialized = {:?}", deserialized);

    println!("\n\n");

    let ss = serde_json::to_string(&tree).unwrap();
    println!("tree = {}", ss)
}
