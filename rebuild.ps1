$ErrorActionPreference = "Stop"

#$env:RUST_TARGET_PATH = $(rustc --print sysroot)
$rust_sysroot = $(rustc --print sysroot)
$env:CARGO_PROFILE_RELEASE_DEBUG = 0
$env:CARGO_PROFILE_RELEASE_OPT_LEVEL = ""
$env:CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS = "true"
$env:RUSTC_BOOTSTRAP = 1
$env:__CARGO_DEFAULT_LIB_METADATA = "stablestd"

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

if (-Not(Test-Path "$dest_path\target.json")) {
    Copy-Item "riscv32imac-unknown-xous-elf.json" "$dest_path\target.json"
}

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
