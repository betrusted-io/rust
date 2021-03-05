# Rust Stable for Xous

Build stable Rust binaries for Xous!

## Usage

1. Copy `riscv32imac-unknown-xous-elf.json` to your Rust sysroot. This can be done on Unix-like systems by running:

```
cp riscv32imac-unknown-xous-elf.json $(rustc --print sysroot)
```

2. Set the `RUST_TARGET_PATH` to point to the sysroot.

```
export RUST_TARGET_PATH=$(rustc --print sysroot)
```

3. Compile the standard library:

```
CARGO_PROFILE_RELEASE_DEBUG=0 \
CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=false \
RUSTC_BOOTSTRAP=1 \
__CARGO_DEFAULT_LIB_METADATA=stablestd \
cargo build \
    --target riscv32imac-unknown-xous-elf \
    -Zbinary-dep-depinfo \
    -j 24 \
    --release \
    --features "panic-unwind backtrace compiler-builtins-c" \
    --manifest-path "library/test/Cargo.toml"
```

4. Install the standard library to your new sysroot:

```
mkdir -p $(rustc --print sysroot)/lib/rustlib/riscv32imac-unknown-xous-elf/lib/
cp target/riscv32imac-unknown-xous-elf/release/deps/*.rlib $(rustc --print sysroot)/lib/rustlib/riscv32imac-unknown-xous-elf/lib/
```

5. Use the new stdlib by setting `--target`:

```
cargo build --target riscv32imac-unknown-xous-elf
```

## Target Files

Rust supports specifying a Json file to define a custom target. 
