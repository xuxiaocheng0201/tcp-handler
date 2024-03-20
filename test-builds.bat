cargo build --no-default-features
cargo build --no-default-features --features "compression"
cargo build --no-default-features --features "encryption"
cargo build --no-default-features --features "compress_encryption"

cargo test --features "serde"
