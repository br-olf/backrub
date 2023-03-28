use backrub::*;
use std::fs;

fn create_argon2_config() -> argon2::Config<'static> {
    let config = argon2::Config {
        ad: &[],
        hash_length: 192,
        lanes: 4,
        mem_cost: 1024 * 1024 * 2, //2GB
        secret: &[],
        thread_mode: argon2::ThreadMode::Parallel,
        time_cost: 1,
        variant: argon2::Variant::Argon2id,
        version: argon2::Version::Version13,
    };
    config
}

fn main() {
    if let Ok(entries) = fs::read_dir(".") {
        for entry in entries {
            if let Ok(entry) = entry {
                // Here, `entry` is a `DirEntry`.
                if let Ok(metadata) = entry.metadata() {
                    // Now let's show our entry's permissions!
                    if let Ok(file_type) = entry.file_type() {
                        println!(
                            "{:?}:\n  {:?}\n   {:?}\n",
                            entry.path(),
                            file_type,
                            metadata
                        );
                    } else {
                        println!("Couldn't get file type for {:?}", entry.path());
                    }
                } else {
                    println!("Couldn't get metadata for {:?}", entry.path());
                }
            }
        }
    }

    if false {
        let passwd = "secret".to_string();
        let salt = "salt".to_string();
        let a2config = create_argon2_config();

        let passwd = bincode::serialize(&passwd).unwrap();
        let salt = blake3::hash(&bincode::serialize(&salt).unwrap());

        let a2hash = argon2::hash_encoded(&passwd, salt.as_bytes(), &a2config).unwrap();

        println!("{a2hash}")
    }
}
