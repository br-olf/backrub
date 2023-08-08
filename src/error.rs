use serde::{Deserialize, Serialize};
use std::{error, fmt, path::PathBuf};

/// Error type for errors that are specific for backrub.
///
/// For all practical purposes this will be wrapped into [Error].
#[derive(Clone, Hash, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum BackrubError {
    SledKeyLengthError,
    SledTreeNotEmpty,
    SledDbAlreadyExists(PathBuf),
    SledDbDidNotExist(PathBuf),
    SelfTestError,
    InvalidSignature,
    BackupRootMustBeDir(PathBuf),
}

impl fmt::Display for BackrubError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            BackrubError::InvalidSignature => {
                write!(
                    f,
                    "InvalidSignature: a signature is invalid, this could be a sign of tampering"
                )
            }
            BackrubError::BackupRootMustBeDir(path) => {
                write!(
                    f,
                    "BackupRootMustBeDir: the root of any backup must be a directory, got \"{}\"",
                    path.display()
                )
            }
            BackrubError::SledDbAlreadyExists(path) => {
                write!(
                    f,
                    "SledDbAlreadyExists: a sled database is already existing at given path \"{}\"",
                    path.display()
                )
            }
            BackrubError::SledDbDidNotExist(path) => {
                write!(
                    f,
                    "SledDbDidNotExist: a sled database was NOT existing at given path \"{}\"",
                    path.display()
                )
            }
            BackrubError::SledTreeNotEmpty => {
                write!(
                    f,
                    "SledTreeNotEmpty: a sled tree has a length bigger than 0"
                )
            }
            BackrubError::SledKeyLengthError => {
                write!(
                    f,
                    "SledKeyLengthError: a sled key seems to be of wrong length"
                )
            }
            BackrubError::SelfTestError => {
                write!(f, "SelfTestError: a sled key - value pair is corrupted")
            }
        }
    }
}

impl error::Error for BackrubError {}

macro_rules! impl_error_enum{
    (
        $(#[$meta:meta])*
        $vis:vis enum $enum_name:ident {
            $(
            $(#[$field_meta:meta])*
            $field_type:ident ( $enc_type:ty )
            ),*$(,)+
        }
    ) => {
        $(#[$meta])*
        $vis enum $enum_name{
            $(
            $(#[$field_meta:meta])*
            $field_type ( $enc_type ),
            )*
        }

        impl fmt::Display for $enum_name {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                match self {
                    $($enum_name::$field_type ( error ) => {
                        write!(f, "{}::{}: ", stringify!($enum_name), stringify!($field_type))?;
                        error.fmt(f)
                    })*
                }
            }
        }

        $(
        impl From<$enc_type> for $enum_name {
           fn from(err: $enc_type) -> Self {
               $enum_name::$field_type(err)
           }
        }
        )*

        impl std::error::Error for $enum_name {}
    }
}

impl_error_enum!(
    /// Encapsulating error type for all possible kinds of errors in backrub.
    ///
    /// This is [Error] is used by [Result].
    #[derive(Debug)]
    pub enum Error {
        CryptoError(chacha20poly1305::aead::Error),
        BackrubError(BackrubError),
        SledError(sled::Error),
        BincodeError(Box<bincode::ErrorKind>),
        IoError(std::io::Error),
        TryFromSliceError(std::array::TryFromSliceError),
        SerdeJsonError(serde_json::Error),
        Argon2Error(argon2::Error),
    }
);

/// Backrub specific result wrapper, using [Error].
pub type Result<T> = std::result::Result<T, Error>;
