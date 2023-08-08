use super::error::*;
use super::structs::*;
use chacha20poly1305::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    XChaCha20Poly1305,
};
use flate2::write::{DeflateDecoder, DeflateEncoder};
use flate2::Compression;
use serde::{Deserialize, Serialize};
use std::io::prelude::*;

#[derive(Clone, Hash, Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CryptoCtx {
    nonce: EncNonce,
    data: Vec<u8>,
}

pub trait Hashable: Serialize {
    fn hash(&self) -> Result<blake3::Hash> {
        let serialized = bincode::serialize(self)?;
        serialized.hash()
    }

    fn keyed_hash(&self, key: &EncKey) -> Result<blake3::Hash> {
        let serialized = bincode::serialize(self)?;
        serialized.keyed_hash(key)
    }
}

impl Hashable for Vec<u8> {
    /// This will never return Err
    fn hash(&self) -> Result<blake3::Hash> {
        let mut hasher = blake3::Hasher::new();
        hasher.update_rayon(&self);
        Ok(hasher.finalize())
    }

    /// This will never return Err
    fn keyed_hash(&self, key: &EncKey) -> Result<blake3::Hash> {
        let mut hasher = blake3::Hasher::new_keyed(key.as_array());
        hasher.update_rayon(&self);
        Ok(hasher.finalize())
    }
}

impl Hashable for &[u8] {
    /// This will never return Err
    fn hash(&self) -> Result<blake3::Hash> {
        let mut hasher = blake3::Hasher::new();
        hasher.update_rayon(&self);
        Ok(hasher.finalize())
    }

    /// This will never return Err
    fn keyed_hash(&self, key: &EncKey) -> Result<blake3::Hash> {
        let mut hasher = blake3::Hasher::new_keyed(key.as_array());
        hasher.update_rayon(&self);
        Ok(hasher.finalize())
    }
}

pub trait Encrypt: Serialize + for<'a> Deserialize<'a> {
    /// Generic function to encrypt data in backrub
    fn encrypt(&self, key: &EncKey) -> Result<Vec<u8>> {
        // convert data to Vec<u8>
        let serialized_data = bincode::serialize(self)?;
        serialized_data.encrypt(key)
    }

    /// Generic function to decrypt data encrypted by backrub
    fn decrypt(data: &[u8], key: &EncKey) -> Result<Self> {
        // decrypt the data
        let data = Vec::<u8>::decrypt(data, key)?;
        // convert decrypted data to the target data type
        Ok(bincode::deserialize(&data)?)
    }

    /// Generic function to compress and encrypt data in backrub
    fn compress_and_encrypt(&self, key: &EncKey) -> Result<Vec<u8>> {
        // convert data to Vec<u8>
        let serialized_data = bincode::serialize(self)?;
        serialized_data.compress_and_encrypt(key)
    }

    /// Generic function to decrypt and uncompress data encrypted by backrub
    fn decrypt_and_uncompress(data: &[u8], key: &EncKey) -> Result<Self> {
        // decrypt and decompress the data
        let data = Vec::<u8>::decrypt_and_uncompress(data, key)?;
        // deserialize uncompressed, decrypted data
        Ok(bincode::deserialize(&data)?)
    }
}

impl Encrypt for Vec<u8> {
    fn encrypt(&self, key: &EncKey) -> Result<Vec<u8>> {
        // generate nonce
        let nonce: EncNonce = XChaCha20Poly1305::generate_nonce(&mut OsRng).into();
        // setup the cipher
        let cipher = XChaCha20Poly1305::new(key.as_array().into());
        // encrypt the data
        let encrypted_data = cipher.encrypt(nonce.as_array().into(), &self[..])?;
        // construct CryptoCtx using the nonce and the encrypted data
        let ctx = CryptoCtx {
            nonce,
            data: encrypted_data,
        };
        // convert CryptoCtx to Vec<u8>
        Ok(bincode::serialize(&ctx)?)
    }

    fn decrypt(data: &[u8], key: &EncKey) -> Result<Self> {
        // decode encrypted data to split nonce and encrypted data
        let ctx = bincode::deserialize::<CryptoCtx>(data)?;
        // setup the cipher
        let cipher = XChaCha20Poly1305::new(key.as_array().into());
        // decrypt the data
        Ok(cipher.decrypt(ctx.nonce.as_array().into(), &ctx.data[..])?)
    }

    fn compress_and_encrypt(&self, key: &EncKey) -> Result<Vec<u8>> {
        // generate nonce
        let nonce: EncNonce = XChaCha20Poly1305::generate_nonce(&mut OsRng).into();
        // setup the cipher
        let cipher = XChaCha20Poly1305::new(key.as_array().into());

        // compress the data
        let mut compressor = DeflateEncoder::new(Vec::new(), Compression::default());
        compressor.write_all(&self[..])?;
        let compressed_data = compressor.finish()?;
        // encrypt the data
        let encrypted_data = cipher.encrypt(nonce.as_array().into(), &compressed_data[..])?;
        // construct CryptoCtx using the nonce and the encrypted data
        let ctx = CryptoCtx {
            nonce,
            data: encrypted_data,
        };
        // convert CryptoCtx to Vec<u8>
        Ok(bincode::serialize(&ctx)?)
    }

    fn decrypt_and_uncompress(data: &[u8], key: &EncKey) -> Result<Self> {
        // decode encrypted data to split nonce and encrypted data
        let ctx = bincode::deserialize::<CryptoCtx>(data)?;
        // setup the cipher
        let cipher = XChaCha20Poly1305::new(key.as_array().into());
        // decrypt the data
        let decrypted_data = cipher.decrypt(ctx.nonce.as_array().into(), &ctx.data[..])?;
        // decompress decrypted data
        let uncompressed_data = Vec::new();
        let mut deflater = DeflateDecoder::new(uncompressed_data);
        deflater.write_all(&decrypted_data)?;
        Ok(deflater.finish()?)
    }
}
