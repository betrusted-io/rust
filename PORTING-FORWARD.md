Use `git log` to figure out where the oldest patch is:

```
$ git pull
$ git log HEAD~52

--  output ended up being too old --
$ git log HEAD~49
Author: Sean Cross <sean@xobs.io>
Date:   Wed May 12 17:38:01 2021 +0800

    repo: Remove dynamic files to keep patching easier
---
$
```

Turn it into a patchset -- be sure to add `1` to the revision:

```
$ git format-patch HEAD~50
0001-repo-Remove-dynamic-files-to-keep-patching-easier.patch
0002-README-replace-Rust-readme-with-our-own-version.patch
0003-json-add-riscv32imac-unknown-xous-elf.json.patch
0004-library-std-add-initial-xous-support.patch
...
$
```

Sync with upstream

```
$ git fetch git@github.com:rust-lang/rust.git
remote: Enumerating objects: 7501, done.
remote: Counting objects: 100% (4728/4728), done.
remote: Total 7501 (delta 4728), reused 4728 (delta 4728), pack-reused 2773
...
```

Install `beta` toolchain and check the version

```
$ rustup update
$ rustc --version
rustc 1.63.0 (7c13df853 2022-04-09)
```

Check out the version of the git repository that matches rustc. In our case, `7c13df853`

```
$ rm .\Cargo.lock
$ git checkout 7c13df853
warning: unable to rmdir 'library/compiler-builtins': Directory not empty
Updating files: 100% (3867/3867), done.
Note: switching to '7c13df853'.
$
```

Check out a new branch.

```
$ git checkout -b 1.63.0-xous
```

Apply all patches and fix as necessary. Note that the first patch will always fail, as these files are constantly changing. These patches are designed to fail, and their solution is always to remove the files in question.

```
$ git am *.patch
error: patch failed: .github/workflows/ci.yml:1
error: .github/workflows/ci.yml: patch does not apply
error: patch failed: Cargo.lock:1
error: Cargo.lock: patch does not apply
hint: Use 'git am --show-current-patch=diff' to see the failed patch
Applying: repo: Remove dynamic files to keep patching easier
Patch failed at 0001 repo: Remove dynamic files to keep patching easier
When you have resolved this problem, run "git am --continue".
If you prefer to skip this patch, run "git am --skip" instead.
To restore the original branch and stop patching, run "git am --abort".
$ git rm Cargo.lock
$ git rm .github/workflows/ci.yml
$ git am --continue
.git/rebase-apply/patch:72: trailing whitespace.
(Get-Content $file_to_patch |
warning: 1 line adds whitespace errors.
Applying: repo: Remove dynamic files to keep patching easier
Applying: README: replace Rust readme with our own version
Applying: json: add riscv32imac-unknown-xous-elf.json
Applying: library: std: add initial xous support
...
```

Push the changes upstream. This will test the build, and will report errors if there's a failure.

```
$ git push -u origin 1.63.0-xous
```

Tag a release and push the tag. This will build and release the package.

```
$ git tag -a 1.63.0.1 -m "xous: Release Xous for Rust 1.63.0"
$ git push --tags
$
```