#!/usr/bin/env bash
# Build the Rust core for Android and regenerate the Kotlin bindings
# (spec 2026-10-01-android-app §12.1). Generated bindings are committed;
# jniLibs are not.
set -euo pipefail
cd "$(dirname "$0")/.."
profile=debug
cargo_flags=()
if [[ "${1:-}" == "--release" ]]; then
  profile=release
  cargo_flags=(--release)
  # The workspace strips release symbols, which removes the metadata
  # uniffi-bindgen reads; the Android build strips the .so when packaging.
  export CARGO_PROFILE_RELEASE_STRIP=debuginfo
fi
: "${ANDROID_NDK_HOME:?set ANDROID_NDK_HOME to the NDK directory}"
jni=apps/android/app/src/main/jniLibs
cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 -o "$jni" \
  build -p havenkeys-mobile "${cargo_flags[@]}"
cargo run -p havenkeys-mobile --features bindgen --bin uniffi-bindgen -- \
  generate --library "target/aarch64-linux-android/$profile/libhavenkeys_mobile.so" \
  --language kotlin --out-dir apps/android/app/src/main/kotlin --no-format
