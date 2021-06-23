# Everything you push to main will do a test build, and let you know if it breaks.
#
# Things only get released if you tag it. And the actual build is based on the tag.
# Without tagging it, nothing is released and it doesn't affect anyone at all, aside
# from people building it from source.
#
# Look at the list of tags:
#
# https://github.com/betrusted-io/rust/tags
#
# We increment the 4th decimal. So far with the 1.59.0 branch, we've had two releases: 1.59.0.1 and 1.59.0.2. If you decided to release a new version of libstd, you would do:
#
# git tag -a 1.59.0.3 # Commit a message, indicating what you've changed
# git push --tags
#
# That would build and release a new version.

$ErrorActionPreference = "Stop"

Function Test-CommandExists {
    Param ($command)
    $oldPreference = $ErrorActionPreference
    $ErrorActionPreference = 'stop'
    try { if (Get-Command $command) { return $true } }
    Catch { return $false }
    Finally { $ErrorActionPreference = $oldPreference }
} #end function test-CommandExists

#$env:RUST_TARGET_PATH = $(rustc --print sysroot)
$rust_sysroot = $(rustc --print sysroot)

$env:RUST_COMPILER_RT_ROOT = "$(Get-Location)\src\llvm-project\compiler-rt"
$env:CARGO_PROFILE_RELEASE_DEBUG = 0
$env:CARGO_PROFILE_RELEASE_OPT_LEVEL = ""
$env:CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS = "true"
$env:RUSTC_BOOTSTRAP = 1
$env:RUSTFLAGS = "-Cforce-unwind-tables=yes -Cembed-bitcode=yes"
$env:__CARGO_DEFAULT_LIB_METADATA = "stablestd"

# Set up the C compiler. We need to explicitly specify these variables
# because the `cc` package obviously doesn't recognize our target triple.
if (Test-CommandExists riscv32-unknown-elf-gcc) {
    $env:CC = "riscv32-unknown-elf-gcc"
    $env:AR = "riscv32-unknown-elf-ar"
}
elseif (Test-CommandExists riscv-none-embed-gcc) {
    $env:CC = "riscv-none-embed-gcc"
    $env:AR = "riscv-none-embed-ar"
}
elseif (Test-CommandExists riscv-none-elf-gcc) {
    $env:CC = "riscv-none-elf-gcc"
    $env:AR = "riscv-none-elf-ar"
}
elseif (Test-CommandExists riscv64-unknown-elf-gcc) {
    $env:CC = "riscv64-unknown-elf-gcc"
    $env:AR = "riscv64-unknown-elf-ar"
}
else {
    throw "No C compiler found for riscv"
}

# Patch llvm's source to not enable `u128` for our platform.
$line_to_remove = "#define CRT_HAS_128BIT"
$file_to_patch = ".\src\llvm-project\compiler-rt\lib\builtins\int_types.h"
(Get-Content $file_to_patch | 
    Where-Object { $_ -notmatch $line_to_remove }) |
    Set-Content $file_to_patch

$src_path = ".\target\riscv32imac-unknown-xous-elf\release\deps"
$dest_path = "$rust_sysroot\lib\rustlib\riscv32imac-unknown-xous-elf"
$dest_lib_path = "$dest_path\lib"
function Get-ItemBaseName {
    param ($ItemName)
    # Write-Host "Item name: $ItemName"
    $sub_strings = $ItemName -split "-"
    $last_string_count = $sub_strings.Count
    $ItemName -replace "-$($sub_strings[$last_string_count-1])", ""
    # return $result
}

if (-Not( Test-Path $dest_lib_path)) {
    New-Item -Path $dest_lib_path -ItemType Directory
}

$(rustc --version).split(" ")[1] | New-Item -Path "$dest_path\RUST_VERSION" -force

# Remove stale objects
Remove-Item "$dest_lib_path\*.rlib"

$previous_libraries = @{}

if (Test-Path $src_path) {
    ForEach ($item in Get-ChildItem "$src_path\*.rlib") {
        $base_string = Get-ItemBaseName ($item.Name)
        # Write-Output "Base string is $base_string"
        if ($previous_libraries.ContainsKey($base_string)) {
            throw "There is a duplicate of $base_string!"
        }
        $previous_libraries.add($base_string, $item.Name)
    }
}

cargo build `
    --target riscv32imac-unknown-xous-elf `
    -Zbinary-dep-depinfo `
    --release `
    --features "panic-unwind compiler-builtins-c compiler-builtins-mem" `
    --manifest-path "library/test/Cargo.toml"
if ($LastExitCode -ne 0) {
    "Cargo exited with $LastExitCode"
}

ForEach ($item in Get-ChildItem "$src_path\*.rlib") {
    $base_string = Get-ItemBaseName ($item.Name)
    # Write-Output "Base string is $base_string"
    if ($previous_libraries.ContainsKey($base_string)) {
        if ($previous_libraries[$base_string] -ne $item.Name) {
            Remove-Item "$src_path\$($previous_libraries[$base_string])"
        }
    }
}

Copy-Item "$src_path\*.rlib" "$dest_lib_path"
