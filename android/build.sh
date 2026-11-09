#!/usr/bin/env bash
# Build the APK: the Rust library for each ABI, then the Java side with Gradle.
#
#   android/build.sh [debug|release] [--install]
#
# debug: the emulator (x86_64) and the phone (arm64), quick Rust build.
# release: optimized Rust (fat LTO, slow to build), for battery and speed on the phone,
# with the dependencies that don't run every frame built for size (profile `android`).
# --install: adb install the APK on the connected device or emulator.
# ABIS="arm64-v8a" builds only the phone's (default: arm64-v8a and x86_64 for debug,
# arm64-v8a alone for release: phones are arm64, x86_64 is the emulator).
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
[[ -f "$HOME/.local/opt/android-env.sh" ]] && source "$HOME/.local/opt/android-env.sh"

profile="${1:-debug}"
install=false
[[ "${2:-}" == "--install" || "${1:-}" == "--install" ]] && install=true
[[ "$profile" == "--install" ]] && profile=debug

cargo_flags=()
case "$profile" in
    debug) ;;
    # The `android` profile: `release`, with cold dependencies built for size (Cargo.toml).
    release) cargo_flags+=(--profile android) ;;
    *) echo "usage: $0 [debug|release] [--install]" >&2; exit 2 ;;
esac

cd "$here/.."
targets=()
default_abis="arm64-v8a x86_64"
[[ "$profile" == release ]] && default_abis="arm64-v8a"
for abi in ${ABIS:-$default_abis}; do targets+=(-t "$abi"); done
# Libraries from an earlier build for other ABIs would end up in the APK too.
rm -rf "$here/app/src/main/jniLibs"
cargo ndk "${targets[@]}" -P 33 -o "$here/app/src/main/jniLibs" build --lib "${cargo_flags[@]}"
# Debug info would make the APK over a gigabyte; the symbols stay for readable crash logs.
strip="$ANDROID_HOME/ndk/29.0.14206865/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-strip"
"$strip" --strip-debug "$here"/app/src/main/jniLibs/*/libytm_rs.so

cd "$here"
apk="$here/app/build/outputs/apk/$profile/app-$profile.apk"
# Packaged incrementally over an old APK, the file keeps the old libraries' size.
rm -f "$apk"
./gradlew --quiet "assemble${profile^}"
echo "APK: $apk ($(du -h "$apk" | cut -f1))"

if $install; then
    adb install -r "$apk"
fi
