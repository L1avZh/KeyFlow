//! Helper binary that turns this crate's compiled UniFFI metadata into
//! Kotlin source. Not shipped anywhere — run on the host only, e.g.:
//!
//!   cargo run --features uniffi-bindgen --bin uniffi-bindgen -- \
//!       generate --library target/debug/libkeyflow_mobile.dylib \
//!       --language kotlin --out-dir ../../android/app/src/main/java

fn main() {
    uniffi::uniffi_bindgen_main()
}
