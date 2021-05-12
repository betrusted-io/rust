# Rust Stable for Xous

Build stable Rust binaries for Xous! This release targets Rust 1.64.0.

## Supported Features

When porting the standard library to a new operating system, a piecemeal approach is taken. Features are gradually brought up as they are needed. So far, the following features work:

* println!()
* sync::Mutex
* sync::Condvar
* net::TcpStream
* net::UdpSocket
* net::LookupHost
* time::Duration
* time::Instant
* thread::sleep
* thread::spawn
* thread::local

## Installing Prebuilt Releases

1. Ensure you are running Rust 1.64.0. Future versions of Rust will need a different version of this software.
2. Download the latest release from the [releases](https://github.com/betrusted-io/rust/releases/latest) page
3. Unzip the zipfile to your Rust sysroot. On Unix systems can do this with something like:
```sh
cd $(rustc --print sysroot)
wget https://github.com/betrusted-io/rust/releases/latest/download/riscv32imac-unknown-xous_1.64.0.zip
rm -rf lib/rustlib/riscv32imac-unknown-xous-elf # Remove any existing version
unzip *.zip
rm *.zip
cd -
```

On Windows with Powershell you can run:

```powershell
Push-Location $(rustc --print sysroot)
if (Test-Path lib\rustlib\riscv32imac-unknown-xous-elf) { Remove-Item -Recurse -Force lib\rustlib\riscv32imac-unknown-xous-elf }
Invoke-WebRequest -Uri https://github.com/betrusted-io/rust/releases/latest/download/riscv32imac-unknown-xous_1.64.0.zip -Outfile toolchain.zip
Expand-Archive -DestinationPath . -Path toolchain.zip
Remove-Item toolchain.zip
Pop-Location
```

## Building From Source

1. Install a RISC-V toolchain, and ensure it's in your path. Set `CC` and `AR` to point to the toolchain's -gcc and -ar binaries.
2. Set `RUST_COMPILER_RT_ROOT` to `$(pwd)/src/llvm-project/compiler-rt`
3. Patch `src/llvm-project/compiler-rt/lib/builtins/int_types.h` to remove `#define CRT_HAS_128BIT`.
4. Copy `riscv32imac-unknown-xous-elf.json` to your Rust sysroot under a new target directory. This can be done on Unix-like systems by running:

```
mkdir -p $(rustc --print sysroot)/lib/rustlib/riscv32imac-unknown-xous-elf/lib
cp riscv32imac-unknown-xous-elf.json $(rustc --print sysroot)/lib/rustlib/riscv32imac-unknown-xous-elf/target.json
```

5. Compile the standard library:

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

6. Install the standard library to your new sysroot:

```
mkdir -p $(rustc --print sysroot)/lib/rustlib/riscv32imac-unknown-xous-elf/lib/
cp target/riscv32imac-unknown-xous-elf/release/deps/*.rlib $(rustc --print sysroot)/lib/rustlib/riscv32imac-unknown-xous-elf/lib/
```

7. Use the new stdlib by setting `--target`:

```
cargo build --target riscv32imac-unknown-xous-elf
```

## Building on Windows Powershell

On Windows, you can use the `rebuild.ps1` script to build and install this package. You will need
to have a Riscv compiler in your path.

Run `rebuild.ps1`. It is recommended that you run it under a new shell in order to avoid polluting your environment with Rust-specific variables:

```powershell
powershell .\rebuild.ps1
```

Alternately, you can run the following commands to manually build things:

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
