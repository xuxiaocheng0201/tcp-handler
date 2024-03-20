//! The protocols.
//! See the sub mod docs for more info.
//!
//! [raw], [compress], [encrypt], and [compress_encrypt].

pub mod common;

pub mod raw;
#[cfg(feature = "compression")]
#[cfg_attr(docsrs, doc(cfg(feature = "compression")))]
pub mod compress;
#[cfg(feature = "encryption")]
#[cfg_attr(docsrs, doc(cfg(feature = "encryption")))]
pub mod encrypt;
#[cfg(feature = "compress_encryption")]
#[cfg_attr(docsrs, doc(cfg(feature = "compress_encryption")))]
pub mod compress_encrypt;
