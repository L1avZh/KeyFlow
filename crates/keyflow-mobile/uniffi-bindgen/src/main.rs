//! Helper binary that turns this crate's compiled UniFFI metadata into
//! Kotlin source. Not shipped anywhere — normally invoked automatically
//! by apps/android/app/build.gradle.kts's `generateUniffiBindings` task
//! (output lands in a Gradle build/ dir, not checked into git). To run
//! it manually on the host for inspection:
//!
//!   cargo run --features uniffi-bindgen --bin uniffi-bindgen -- \
//!       generate --library target/debug/libkeyflow_mobile.dylib \
//!       --language kotlin --out-dir /tmp/keyflow-mobile-bindgen

fn main() {
    uniffi::uniffi_bindgen_main()
}
