#!/bin/bash
# Build ion shell with Cranelift for aarch64 Redox
set -e

cd "$(dirname "$0")"

NIGHTLY="nightly-2026-01-02"
TARGET="aarch64-unknown-redox-clif.json"
CRANELIFT="/opt/other/rustc_codegen_cranelift/dist/lib/librustc_codegen_cranelift.dylib"
SYSROOT="/opt/other/redox/build/aarch64/sysroot/lib"

export DYLD_LIBRARY_PATH=~/.rustup/toolchains/${NIGHTLY}-aarch64-apple-darwin/lib

export RUSTFLAGS="-Zcodegen-backend=${CRANELIFT} \
  -Crelocation-model=static \
  -Clink-arg=-L${SYSROOT} \
  -Clink-arg=${SYSROOT}/crt0.o \
  -Clink-arg=${SYSROOT}/crti.o \
  -Clink-arg=${SYSROOT}/crtn.o \
  -Clink-arg=-z -Clink-arg=muldefs \
  -Cpanic=abort"

echo "=== Building ion shell with Cranelift ==="
cargo +${NIGHTLY} build \
    --target ${TARGET} \
    --release \
    -Z build-std=core,alloc,std,panic_abort \
    -Zbuild-std-features=compiler_builtins/no-f16-f128 \
    --config net.offline=false

echo "=== Stripping ==="
OUT=/tmp/ion-build
mkdir -p $OUT
llvm-strip -o "$OUT/ion" "target/aarch64-unknown-redox-clif/release/ion"
echo "  ion -> $OUT/ion"

echo "=== Done ==="
ls -la $OUT/ion
file $OUT/ion
