#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod config;
pub mod common;

pub mod raw;
#[cfg(feature = "compression")]
#[cfg_attr(docsrs, doc(cfg(feature = "compression")))]
pub mod compress;
// #[cfg(feature = "encryption")]
// #[cfg_attr(docsrs, doc(cfg(feature = "encryption")))]
// pub mod encrypt;
// #[cfg(all(feature = "compression", feature = "encryption"))]
// #[cfg_attr(docsrs, doc(cfg(all(feature = "compression", feature = "encryption"))))]
// pub mod compress_encrypt;

pub extern crate bytes;

#[cfg(feature = "compression")]
pub use flate2::Compression;
