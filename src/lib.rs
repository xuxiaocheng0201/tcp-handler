#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![forbid(unsafe_code)]
#![forbid(missing_docs)]

pub mod config;
pub mod protocols;
#[cfg(feature = "streams")]
#[cfg_attr(docsrs, doc(cfg(feature = "streams")))]
pub mod streams;


#[cfg(feature = "streams")]
pub use streams::*;

pub extern crate bytes;

#[cfg(feature = "compression")]
pub use flate2::Compression;
