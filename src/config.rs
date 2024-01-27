//! Global configuration for this crate.
//!
//! You may change the configuration by calling `set_config` function.
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
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Config {
    /// `max_packet_size` is the maximum size of a packet in bytes.
    /// It is used to limit the size of a packet that can be received or sent.
    /// Default value is `1 << 20`.
    ///
    /// # Example
    /// ```rust
    /// use tcp_handler::config::Config;
    ///
    /// # fn main() {
    /// let config = Config {
    ///     max_packet_size: 1 << 10,
    /// };
    /// # let _ = config;
    /// # }
    /// ```
    pub max_packet_size: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_packet_size: 1 << 20,
        }
    }
}

static CONFIG: RwLock<Config> = RwLock::new(Config {
    max_packet_size: 1 << 20,
});

/// Sets the global configuration.
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

/// Gets the global configuration.
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

#[cfg(test)]
mod test {
    use crate::config::{Config, get_max_packet_size, set_config};

    #[test]
    fn get() {
        let _ = get_max_packet_size();
    }

    #[test]
    fn set() {
        set_config(Config { max_packet_size: 1 });
        assert_eq!(1, get_max_packet_size());
    }

    #[test]
    fn set_twice() {
        set_config(Config { max_packet_size: 1 });
        assert_eq!(1, get_max_packet_size());
        set_config(Config { max_packet_size: 2 });
        assert_eq!(2, get_max_packet_size());
    }
}
