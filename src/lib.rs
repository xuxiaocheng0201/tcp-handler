#![doc = include_str!("../README.md")]
#![allow(unused_imports)] // For features.

pub mod config;
mod packet;
mod starter;

#[cfg(feature = "encrypt")]
pub use crate::starter::AesCipher;

pub mod raw {
    pub use crate::starter::client_init;
    pub use crate::starter::server_init;
    pub use crate::starter::server_start;
    pub use crate::starter::client_start;
    pub use crate::packet::send;
    pub use crate::packet::recv;
}

#[cfg(feature = "compression")]
pub mod compress {
    pub use crate::starter::client_init_with_compress as client_init;
    pub use crate::starter::server_init_with_compress as server_init;
    pub use crate::starter::server_start_with_compress as server_start;
    pub use crate::starter::client_start_with_compress as client_start;
    pub use crate::packet::send_with_compress as send;
    pub use crate::packet::recv_with_compress as recv;
}

#[cfg(feature = "encrypt")]
pub mod encrypt {
    pub use crate::starter::client_init_with_encrypt as client_init;
    pub use crate::starter::server_init_with_encrypt as server_init;
    pub use crate::starter::server_start_with_encrypt as server_start;
    pub use crate::starter::client_start_with_encrypt as client_start;
    pub use crate::packet::send_with_encrypt as send;
    pub use crate::packet::recv_with_encrypt as recv;
}

#[cfg(all(feature = "compression", feature = "encrypt"))]
pub mod compress_encrypt {
    pub use crate::starter::client_init_with_compress_and_encrypt as client_init;
    pub use crate::starter::server_init_with_compress_and_encrypt as server_init;
    pub use crate::starter::server_start_with_compress_and_encrypt as server_start;
    pub use crate::starter::client_start_with_compress_and_encrypt as client_start;
    pub use crate::packet::send_with_compress_and_encrypt as send;
    pub use crate::packet::recv_with_compress_and_encrypt as recv;
}

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
