//! Global configuration for this [crate].
//!
//! You may change the configuration by calling [set_config] function.
//!
//! # Example
//! ```rust
//! use tcp_handler::config::{Config, set_config};
//!
//! # fn main() {
//! set_config(Config::default());
//! # }
//! ```

use std::sync::RwLock;
use once_cell::sync::Lazy;

/// Global configuration.
///
/// # Example
/// ```rust
/// use tcp_handler::config::Config;
///
/// # fn main() {
/// let config = Config::default();
/// # let _ = config;
/// # }
/// ```
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Config {
    /// `max_packet_size` is the maximum size of a packet in bytes.
    /// It is used to limit the size of a packet that can be received or sent.
    ///
    /// Default value is `1 << 20`.
    ///
    /// # Example
    /// ```rust
    /// use tcp_handler::config::Config;
    ///
    /// # fn main() {
    /// let config = Config {
    ///     max_packet_size: 1 << 10,
    ///     ..Config::default()
    /// };
    /// # let _ = config;
    /// # }
    /// ```
    pub max_packet_size: usize,

    /// `compression` is the [flate2::Compression] level when sending packets.
    ///
    /// # Example
    /// ```rust
    /// use tcp_handler::config::Config;
    /// use tcp_handler::Compression;
    ///
    /// # fn main() {
    /// let config = Config {
    ///     compression: Compression::fast(),
    ///     ..Config::default()
    /// };
    /// # let _ = config;
    /// # }
    /// ```
    #[cfg(feature = "compression")]
    #[cfg_attr(docsrs, cfg(feature = "compression"))]
    #[cfg_attr(feature = "serde", serde(serialize_with = "serialize_compression", deserialize_with = "deserialize_compression"))]
    pub compression: flate2::Compression,
}

#[cfg(feature = "serde")]
fn serialize_compression<S: serde::Serializer>(compression: &flate2::Compression, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_u32(compression.level())
}

#[cfg(feature = "serde")]
fn deserialize_compression<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<flate2::Compression, D::Error> {
    <u32 as serde::Deserialize>::deserialize(deserializer).map(|l| flate2::Compression::new(l))
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_packet_size: 1 << 20,
            #[cfg(feature = "compression")]
            compression: flate2::Compression::default(),
        }
    }
}

static CONFIG: Lazy<RwLock<Config>> = Lazy::new(|| RwLock::new(Config::default()));

/// Set the global configuration.
///
/// This function is recommended to only be called once during initialization.
///
/// # Example
/// ```rust
/// use tcp_handler::config::{Config, set_config};
///
/// # fn main() {
/// set_config(Config::default());
/// # }
/// ```
#[inline]
pub fn set_config(config: Config) {
    let mut c = CONFIG.write().unwrap();
    *c = config;
}

/// Get the global configuration.
///
/// # Example
/// ```rust
/// use tcp_handler::config::get_config;
///
/// # fn main() {
/// let config = get_config();
/// # let _ = config;
/// # }
/// ```
#[inline]
pub fn get_config() -> Config {
    let c = CONFIG.read().unwrap();
    (*c).clone()
}

/// A cheaper shortcut of
/// ```rust,ignore
/// get_config().max_packet_size
/// ```
#[inline]
pub fn get_max_packet_size() -> usize {
    let c = CONFIG.read().unwrap();
    (*c).max_packet_size
}

/// A cheaper shortcut of
/// ```rust,ignore
/// get_config().compression
/// ```
#[cfg(feature = "compression")]
#[cfg_attr(docsrs, cfg(feature = "compression"))]
#[inline]
pub fn get_compression() -> flate2::Compression {
    let c = CONFIG.read().unwrap();
    (*c).compression
}

#[cfg(test)]
mod test {
    use crate::config::*;

    #[test]
    fn get() {
        let _ = get_max_packet_size();
        let _ = get_compression();
    }

    #[test]
    fn set() {
        set_config(Config { max_packet_size: 1 << 10, ..Config::default() });
        assert_eq!(1 << 10, get_max_packet_size());
    }

    #[test]
    fn set_twice() {
        set_config(Config { max_packet_size: 1 << 10, ..Config::default() });
        assert_eq!(1 << 10, get_max_packet_size());
        set_config(Config { max_packet_size: 2 << 10, ..Config::default() });
        assert_eq!(2 << 10, get_max_packet_size());
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde() {
        let config = Config { max_packet_size: 1 << 10, ..Config::default() };
        let serialized = serde_json::to_string(&config).unwrap();
        let deserialized: Config = serde_json::from_str(&serialized).unwrap();
        assert_eq!(config, deserialized);
    }
}
