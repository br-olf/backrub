use std::fs::File;
use hash_roll::fastcdc;
use hash_roll::Chunk;
use std::io::{BufReader, BufRead, Read, Write};
//use std::prelude::*;
use hash_roll::ChunkIncr;
use memmap::Mmap;
use dedup::*;
use std::collections::{BTreeMap, BTreeSet};

fn some_chunk() -> impl Chunk {
    fastcdc::FastCdc::default()
}


fn main(){
    let mut chunk_iter = fastcdc::FastCdcIncr::default();
    let mut f = File::open("testfile.bin").unwrap();

    let mmap = unsafe { Mmap::map(&f).unwrap()  };

    let mut anztree = BTreeMap::<[u64;4],usize>::new();
    let mut sizetree = BTreeMap::<[u64;4],usize>::new();

    for chunk in chunk_iter.iter_slices(&mmap[..]) {
        let cv = chunk.to_vec();


        let h = *convert_32u8_to_4u64(blake3::hash(chunk).as_bytes());

        match anztree.get(&h) {
            Some(count) => anztree.insert(h, count+1),
            None  => anztree.insert(h, 1),
        };
        sizetree.insert(h, cv.len());
    }

    for (k,si) in sizetree{

        println!("anz: {}    size: {}", anztree.get(&k).unwrap(), si);

    }
}

fn main2() {
    let f = File::open("testfile.bin").unwrap();
    let metadata = f.metadata().unwrap();
    let mut br = BufReader::new(f);
    let chunk = some_chunk();
    let mut ss = chunk.to_search_state();
    let orig_data_len = metadata.len();
    loop
    {
        let data = br.fill_buf().unwrap();
        let len_data = data.len();
        println!("Bytes in Buf: {}", len_data);

        if len_data > 0 {

            let (chunk_begin_opt, chunk_end) = chunk.find_chunk_edge(&mut ss, data);
            match chunk_begin_opt {
                Some(chunk_begin) => { println!("found chunk from: {} - {}", chunk_begin, chunk_end); }
                None => { println!("Discarding {} bytes", chunk_end); }
            }
            br.consume(chunk_end);
        }
        else {
            break;
        }
    }

/*
    loop {

    match chunk {
        Some(cut_point) => {
            // map `cut_point` from the current slice back into the original slice so we can
            // have consistent indexes
            let g_cut = cut_point + orig_data.len() - data.len();
            println!("chunk: {:?}", &orig_data[prev_cut..cut_point]);
        },
        None => {
            println!("no chunk, done with data we have");
            println!("remain: {:?}", &data[discard_ct..]);
            break;
        }
    }

    data = &data[discard_ct..];
    }
    */
}
