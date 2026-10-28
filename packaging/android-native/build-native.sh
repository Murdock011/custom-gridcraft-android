#!/bin/sh
# Builds the native GridCraft APK: Rust .so per ABI with cargo-ndk, then Gradle.
# Usage: sh packaging/android-native/build-native.sh [abis...]   (default: x86_64)
set -eu
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
ABIS="${*:-x86_64}"
ARGS=""
for abi in $ABIS; do ARGS="$ARGS -t $abi"; done
export ANDROID_HOME="${ANDROID_HOME:-$HOME/Android/Sdk}"
export ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-$ANDROID_HOME/ndk/27.2.12479018}"
cd "$ROOT"
# shellcheck disable=SC2086
cargo ndk $ARGS -o "$ROOT/packaging/android-native/app/src/main/jniLibs" build --release -p gridcraft-android
cd "$ROOT/packaging/android-native"
./gradlew assembleDebug
