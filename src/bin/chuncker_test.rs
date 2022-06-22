use hash_roll::fastcdc;
use std::fs::File;
use std::io::{BufRead, BufReader};
//use std::prelude::*;
use hash_roll::ChunkIncr;
use memmap::Mmap;
//use dedup::*;

fn get_first_chunks(buff: &[u8]) -> (Vec<Vec<u8>>, Vec<u8>) {
    let chunk_iter = fastcdc::FastCdcIncr::default();
    let mut iter_slices = chunk_iter.iter_slices_strict(buff);

    let mut chunks = Vec::<Vec<u8>>::new();

    while let Some(chunk) = iter_slices.next() {
        chunks.push(chunk.to_vec());
    }

    let rest = iter_slices.by_ref().take_rem().to_vec();

    (chunks, rest)
}

fn get_chunks(buff: &[u8], rest: &[u8]) -> (Vec<Vec<u8>>, Vec<u8>) {
    get_first_chunks(&[rest, buff].concat())
}

fn chunk_using_buf_reader(fname: String) -> Vec<Vec<u8>> {
    let f = File::open(fname).unwrap();
    let fsize = f.metadata().unwrap().len();
    let mut breader = BufReader::with_capacity(1usize << 20, f);

    let fbuff = breader.fill_buf().unwrap();
    let fbuff_len = fbuff.len();

    let mut cnew: Vec<Vec<u8>>;

    let (mut chunks, mut rest) = get_first_chunks(fbuff);
    breader.consume(fbuff_len);

    while let Ok(buff) = breader.fill_buf() {
        let buff_len = buff.len();
        if buff_len == 0 {
            break;
        }
        (cnew, rest) = get_chunks(buff, &rest[..]);
        chunks.append(&mut cnew);
        breader.consume(buff_len);
    }
    chunks.push(rest.to_vec());

    let mut sizesum: usize = 0;
    let anzchunks = chunks.len();
    for chunk in chunks.clone() {
        sizesum += chunk.len();
    }

    println!("chunked size total: {}", sizesum);
    println!("file size metadata: {}", fsize);
    println!("anz chunks total: {}", anzchunks);

    chunks
}

fn chunk_using_mmap(fname: String) -> Vec<Vec<u8>> {
    let f = File::open(fname).unwrap();
    let fsize = f.metadata().unwrap().len();

    let chunk_iter = fastcdc::FastCdcIncr::default();
    let mmap = unsafe { Mmap::map(&f).unwrap() };

    let mut chunks = Vec::<Vec<u8>>::new();

    for chunk in chunk_iter.iter_slices(&mmap[..]) {
        let cv = chunk.to_vec();
        chunks.push(cv);
    }

    let mut sizesum: usize = 0;
    let anzchunks = chunks.len();
    for chunk in chunks.clone() {
        sizesum += chunk.len();
    }

    println!("chunked size total: {}", sizesum);
    println!("file size metadata: {}", fsize);
    println!("anz chunks total: {}", anzchunks);

    chunks
}

fn main() {
    let fname = std::env::args().nth(1).expect("no filename given");

    println!();
    println!("########################################################");
    println!("############## BufReader ###############################");
    println!("########################################################");
    let c1 = chunk_using_buf_reader(fname.clone());

    println!();
    println!("########################################################");
    println!("############## MMAP ####################################");
    println!("########################################################");
    let c2 = chunk_using_mmap(fname);

    println!();
    println!("########################################################");
    println!("############## BufReader == MMAP #######################");
    if c1 == c2 {
        println!("#################### TRUE ##############################");
    } else {
        println!("#################### FALSE #############################");
    }
    println!("########################################################");
}
