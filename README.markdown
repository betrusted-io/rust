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
    --release \
    --features "panic-unwind backtrace compiler-builtins-c compiler-builtins-mem" \
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
