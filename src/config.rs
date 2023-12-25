use std::hint::spin_loop;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct Config {
    pub max_packet_size: usize,
}

static mut CONFIG: Config = Config {
    max_packet_size: 1 << 20,
};

static STATE: AtomicBool = AtomicBool::new(false);

pub fn set_config(config: Config) {
    loop {
        if match STATE.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst, ) { Ok(s) | Err(s) => s, } {
            while STATE.load(Ordering::SeqCst) {
                spin_loop();
            }
            continue
        } else {
            unsafe { CONFIG = config; }
            STATE.store(false, Ordering::SeqCst);
            break
        };
    };
}

pub fn get_max_packet_size() -> usize {
    loop {
        if match STATE.compare_exchange(false, false, Ordering::SeqCst, Ordering::SeqCst, ) { Ok(s) | Err(s) => s, } {
            while STATE.load(Ordering::SeqCst) {
                spin_loop();
            }
            continue
        } else {
            return unsafe { CONFIG.max_packet_size }
        };
    };
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
