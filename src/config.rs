use std::hint::spin_loop;
use std::sync::atomic::{AtomicUsize, Ordering};
use thiserror::Error;

pub struct Config {
    max_packet_size: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_packet_size: 1 << 20,
        }
    }
}

pub(crate) static mut CONFIG: Config = Config::default();

#[derive(Error, Debug)]
#[error("Already initialized.")]
pub struct InitConfigError {
}

static STATE: AtomicUsize = AtomicUsize::new(0);

const UNINITIALIZED: usize = 0;
const INITIALIZING: usize = 1;
const INITIALIZED: usize = 2;

pub fn init_config(config: Config) -> Result<(), InitConfigError> {
    let old_state = match STATE.compare_exchange(
        UNINITIALIZED,
        INITIALIZING,
        Ordering::SeqCst,
        Ordering::SeqCst,
    ) {
        Ok(s) | Err(s) => s,
    };
    match old_state {
        UNINITIALIZED => {
            unsafe {
                CONFIG = config;
            }
            STATE.store(INITIALIZED, Ordering::SeqCst);
            Ok(())
        }
        INITIALIZING => {
            while STATE.load(Ordering::SeqCst) == INITIALIZING {
                spin_loop();
            }
            Err(InitConfigError {})
        }
        _ => {
            Err(InitConfigError {})
        }
    }
}
