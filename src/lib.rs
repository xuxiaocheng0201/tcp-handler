#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

pub mod config;
pub mod common;

pub mod raw;
#[cfg(feature = "compression")]
pub mod compress;
#[cfg(feature = "encrypt")]
pub mod encrypt;
#[cfg(all(feature = "compression", feature = "encrypt"))]
pub mod compress_encrypt;

pub extern crate bytes;
pub extern crate variable_len_reader;
#[cfg(feature = "compression")]
pub extern crate flate2;
#[cfg(feature = "encrypt")]
pub extern crate rsa;
#[cfg(feature = "encrypt")]
pub extern crate aead;
#[cfg(feature = "encrypt")]
pub extern crate aes_gcm;
#[cfg(feature = "encrypt")]
pub extern crate sha2;
