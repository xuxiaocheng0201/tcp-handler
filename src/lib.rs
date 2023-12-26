#![doc = include_str!("../README.md")]

pub mod config;
pub mod packet;
pub mod starter;

pub extern crate bytes;
pub extern crate variable_len_reader;
#[cfg(feature = "compression")]
pub extern crate flate2;
#[cfg(feature = "encrypt")]
pub extern crate aes_gcm;
