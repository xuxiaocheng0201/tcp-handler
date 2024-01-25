use std::time::Duration;
use reportme::report_build;

fn main() {
    report_build("https://crateio.xxc-metrics.pages.dev/metrics",
                 Duration::from_secs(10),
                 env!("CARGO_PKG_NAME"),
                 env!("CARGO_PKG_VERSION"));
}
