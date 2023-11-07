# Rust Stable for Xous

Build stable Rust binaries for Xous! This release targets Rust 1.77.0.

## Installing Prebuilt Releases

1. Ensure you are running Rust 1.77.0. Future versions of Rust will need a different version of this software.
2. Download the latest release from the [releases](https://github.com/betrusted-io/rust/releases/latest) page
3. Unzip the zipfile to your Rust sysroot. On Unix systems can do this with something like:
```sh
cd $(rustc --print sysroot)
wget https://github.com/betrusted-io/rust/releases/latest/download/riscv32imac-unknown-xous_1.77.0.zip
rm -rf lib/rustlib/riscv32imac-unknown-xous-elf # Remove any existing version
unzip *.zip
rm *.zip
cd -
```

On Windows with Powershell you can run:

```powershell
Push-Location $(rustc --print sysroot)
if (Test-Path lib\rustlib\riscv32imac-unknown-xous-elf) { Remove-Item -Recurse -Force lib\rustlib\riscv32imac-unknown-xous-elf }
Invoke-WebRequest -Uri https://github.com/betrusted-io/rust/releases/latest/download/riscv32imac-unknown-xous_1.77.0.zip -Outfile toolchain.zip
Expand-Archive -DestinationPath . -Path toolchain.zip
Remove-Item toolchain.zip
Pop-Location
```

## Building From Source

1. Install a RISC-V toolchain, and ensure it's in your path. Set `CC` and `AR` to point to the toolchain's -gcc and -ar binaries.
2. Run `./rebuild.sh`. This will build libstd and install it.

## Building on Windows Powershell

On Windows, you can use the `rebuild.ps1` script to build and install this package. You will need
to have a Riscv compiler in your path.

Run `rebuild.ps1`. It is recommended that you run it under a new shell in order to avoid polluting your environment with Rust-specific variables:

```powershell
powershell .\rebuild.ps1
```
