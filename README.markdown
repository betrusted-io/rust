# Rust Stable for Xous

Build stable Rust binaries for Xous! This release targets Rust 1.54.0.

## Installing Prebuilt Releases

1. Ensure you are running Rust 1.54.0. Future versions of Rust will need a different version of this software.
2. Download the latest release from the [releases](https://github.com/betrusted-io/rust/releases/latest) page
3. Unzip the zipfile to your Rust sysroot. You can do this with something like:
```sh
cd $(rustc --print sysroot)
wget https://github.com/betrusted-io/rust/releases/latest/download/riscv32imac-unknown-xous_1.54.0.zip
rm -rf lib/rustlib/riscv32imac-unknown-xous-elf # Remove any existing version
unzip *.zip
rm *.zip
cd -
```

## Building From Source

1. Install a RISC-V toolchain, and ensure it's in your path. Set `CC` and `AR` to point to the toolchain's -gcc and -ar binaries.
2. Patch `src/llvm-project/compiler-rt/lib/builtins/int_types.h` to remove `#define CRT_HAS_128BIT`.
3. Copy `riscv32imac-unknown-xous-elf.json` to your Rust sysroot under a new target directory. This can be done on Unix-like systems by running:

```
mkdir -p $(rustc --print sysroot)/lib/rustlib/riscv32imac-unknown-xous-elf/lib
cp riscv32imac-unknown-xous-elf.json $(rustc --print sysroot)/lib/rustlib/riscv32imac-unknown-xous-elf/target.json
```

4. Compile the standard library:

```
CARGO_PROFILE_RELEASE_DEBUG=0 \
CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=false \
RUSTC_BOOTSTRAP=1 \
__CARGO_DEFAULT_LIB_METADATA=stablestd \
cargo build \
    --target riscv32imac-unknown-xous-elf \
    -Zbinary-dep-depinfo \
    --release \
    --features "panic-unwind backtrace compiler-builtins-c compiler-builtins-mem" \
    --manifest-path "library/test/Cargo.toml"
```

5. Install the standard library to your new sysroot:

```
mkdir -p $(rustc --print sysroot)/lib/rustlib/riscv32imac-unknown-xous-elf/lib/
cp target/riscv32imac-unknown-xous-elf/release/deps/*.rlib $(rustc --print sysroot)/lib/rustlib/riscv32imac-unknown-xous-elf/lib/
```

6. Use the new stdlib by setting `--target`:

```
cargo build --target riscv32imac-unknown-xous-elf
```

## Building on Windows Powershell

```powershell
$env:RUST_TARGET_PATH=$(rustc --print sysroot)
Copy-Item riscv32imac-unknown-xous-elf.json $env:RUST_TARGET_PATH
$env:CARGO_PROFILE_RELEASE_DEBUG=0
$env:CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS="false"
$env:RUSTC_BOOTSTRAP=1
$env:__CARGO_DEFAULT_LIB_METADATA="stablestd"
Remove-Item .\target\riscv32imac-unknown-xous-elf\release\deps\*.rlib
cargo build `
    --target riscv32imac-unknown-xous-elf `
    -Zbinary-dep-depinfo `
    --release `
    --features "panic-unwind backtrace compiler-builtins-c compiler-builtins-mem" `
    --manifest-path "library/test/Cargo.toml"
New-Item -Type Directory -Path "$env:RUST_TARGET_PATH\lib\rustlib\riscv32imac-unknown-xous-elf\lib"
Remove-Item "$env:RUST_TARGET_PATH\lib\rustlib\riscv32imac-unknown-xous-elf\lib\*.rlib"
Copy-Item target\riscv32imac-unknown-xous-elf\release\deps\*.rlib "$env:RUST_TARGET_PATH\lib\rustlib\riscv32imac-unknown-xous-elf\lib"
```

## Target Files

Rust supports specifying a Json file to define a custom target.
