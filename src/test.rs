use super::*;
use crate::{db::*, error::*, manager::*, structs::*, traits::*};
use chacha20poly1305::{
    aead::{rand_core::RngCore, Aead, KeyInit, OsRng},
    XChaCha20Poly1305,
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

#[test]
fn test_compressed_encryption_success() {
    let mut data = Vec::<u8>::new();
    for n in 0..1024 * 1024 {
        for i in 0..5 {
            data.push(i);
        }
    }
    let testdata = Chunk { data };
    let key = XChaCha20Poly1305::generate_key(&mut OsRng);

    let now = Instant::now();

    let enc = testdata.compress_and_encrypt(&key.into()).unwrap();
    let dec = Chunk::decrypt_and_uncompress(&enc, &key.into()).unwrap();

    let elapsed = now.elapsed();

    println!("0: size data uncompressed: {}", testdata.data.len());
    println!("0: size data compressed and encrypted: {}", enc.len());
    println!("0: runtime in nanoseconds: {}", elapsed.as_nanos());
    assert_eq!(testdata, dec);
}

#[test]
fn test_compressed_encryption_fail_tempering() {
    let mut data = Vec::<u8>::new();
    for n in 0..1024 * 1024 {
        for i in 0..5 {
            data.push(i);
        }
    }
    let testdata = Chunk { data };

    let key = XChaCha20Poly1305::generate_key(&mut OsRng);
    let mut enc = testdata.compress_and_encrypt(&key.into()).unwrap();
    let last = enc.pop().unwrap();
    enc.push(!last);

    let dec = Chunk::decrypt_and_uncompress(&enc, &key.into());
    assert!(dec.is_err());
}

#[test]
fn test_compressed_encryption_fail_key() {
    let mut data = Vec::<u8>::new();
    for n in 0..1024 * 1024 {
        for i in 0..5 {
            data.push(i);
        }
    }
    let testdata = Chunk { data };

    let mut key = XChaCha20Poly1305::generate_key(&mut OsRng);
    let enc = testdata.compress_and_encrypt(&key.into()).unwrap();
    key[0] = !key[0];

    let dec = Chunk::decrypt_and_uncompress(&enc, &key.into());
    assert!(dec.is_err());
}

#[test]
fn test_encryption_success() {
    let mut data = Vec::<u8>::new();
    for n in 0..1024 * 1024 {
        for i in 0..5 {
            data.push(i);
        }
    }
    let testdata = Chunk { data };
    let key = XChaCha20Poly1305::generate_key(&mut OsRng);

    let now = Instant::now();

    let enc = testdata.encrypt(&key.into()).unwrap();
    let dec = Chunk::decrypt(&enc, &key.into()).unwrap();

    let elapsed = now.elapsed();

    println!("1: size data uncompressed: {}", testdata.data.len());
    println!("1: size data encrypted: {}", enc.len());
    println!("1: runtime in nanoseconds: {}", elapsed.as_nanos());
    assert_eq!(testdata, dec);
}

#[test]
fn test_encryption_fail_tempering() {
    let mut data = Vec::<u8>::new();
    for n in 0..1024 * 1024 {
        for i in 0..5 {
            data.push(i);
        }
    }
    let testdata = Chunk { data };
    let key = XChaCha20Poly1305::generate_key(&mut OsRng);
    let mut enc = testdata.encrypt(&key.into()).unwrap();
    let last = enc.pop().unwrap();
    enc.push(!last);

    let dec = Chunk::decrypt(&enc, &key.into());
    assert!(dec.is_err());
}

#[test]
fn test_encryption_fail_key() {
    let mut data = Vec::<u8>::new();
    for n in 0..1024 * 1024 {
        for i in 0..5 {
            data.push(i);
        }
    }
    let testdata = Chunk { data };
    let mut key = XChaCha20Poly1305::generate_key(&mut OsRng);
    let enc = testdata.encrypt(&key.into()).unwrap();
    key[0] = !key[0];

    let dec = Chunk::decrypt(&enc, &key.into());
    assert!(dec.is_err());
}

#[test]
fn test_FilePathGen() {
    let mut s = std::collections::HashSet::<String>::new();
    let mut fg = FilePathGen::default();
    for _ in 0..256 {
        for i in 0..1024 {
            let next = fg.next().unwrap();
            s.insert(next);
        }
    }
    assert_eq!(s.len(), 256 * 1024);
    assert_eq!(fg, FilePathGen { 0: 256 * 1024 });

    let mut fg = FilePathGen::from(!0u64 - 1);
    assert_eq!(fg.next(), Some(String::from("ff/ff/ff/ff/ff/ff/ff/ff.bin")));
    assert_eq!(fg.next(), None);
}

#[test]
fn test_log2u64() {
    assert_eq!(log2u64(0u64), None);
    assert_eq!(log2u64(!0u64), Some(63u64));
    assert_eq!(log2u64(1u64), Some(0u64));
    assert_eq!(log2u64(2u64), Some(1u64));
    assert_eq!(log2u64(64u64), Some(6u64));
    assert_eq!(log2u64(63u64), Some(5u64));
}

#[test]
fn test_ChunkDb() {
    let key = EncKey::from(*blake3::hash(b"foobar").as_bytes());
    let config = sled::Config::new().temporary(true);
    let db = config.open().unwrap();

    let mut cs = ChunkDb::new(db.open_tree(b"test").unwrap(), key).unwrap();
    let h1 = ChunkHash::from(*blake3::hash(b"foo").as_bytes());
    let h2 = ChunkHash::from(*blake3::hash(b"bar").as_bytes());
    let h3 = ChunkHash::from(*blake3::hash(b"baz").as_bytes());
    let h4 = ChunkHash::from(*blake3::hash(b"foobar").as_bytes());

    assert_eq!(cs.insert(&h1).unwrap(), (1, PathBuf::from("1.bin")));
    assert_eq!(cs.insert(&h2).unwrap(), (1, PathBuf::from("2.bin")));
    assert_eq!(cs.insert(&h3).unwrap(), (1, PathBuf::from("3.bin")));

    assert_eq!(cs.insert(&h2).unwrap(), (2, PathBuf::from("2.bin")));
    assert_eq!(cs.insert(&h3).unwrap(), (2, PathBuf::from("3.bin")));

    assert_eq!(cs.remove(&h1).unwrap(), Some((0, PathBuf::from("1.bin"))));
    assert_eq!(cs.remove(&h1).unwrap(), None);

    assert_eq!(cs.insert(&h4).unwrap(), (1, PathBuf::from("1.bin")));
}

#[test]
fn test_CryptoKeys_encryption() {
    let ck = CryptoKeys::new();

    let mut raw_keys = [0u8; CRYPTO_KEYS_SIZE];
    OsRng.fill_bytes(&mut raw_keys);
    let kek = KeyEncryptionKeys::from(raw_keys);

    let enc_keys = ck.encrypt(kek.clone());
    let dec_keys = enc_keys.decrypt(kek);

    assert_eq!(ck, dec_keys);
}
