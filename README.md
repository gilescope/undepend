# undepend


## What undepend does:

undepend brute-force removes dependencies one by one by checking that everything compiles after removal
using `cargo check --all-targets`.

It creates a file `unused.sh` with the list of crates that can be removed.
You can trust it by running the script or cat the file in a vscode terminal
and go through the Cargo.toml hyperlinks to check whether you agree that those crates should be removed.

## Installation:

**assumes you have git**

Install undepend and prerequisites:
```sh
cargo install ripgrep cargo-edit undepend
```

## Usage:

Change your current dir to the dir of a freshly checked out git clone and run:
```sh
undepend
```
The tool will iterate over the workspace's dependencies,
removing them individually and establishing if everything still compiles.

** The tool will abort if the checkout is not clean at the start. **

A recorded list of dependencies are written to `unused.sh`

## How udepend works:

cargo metadata is used to understand which crates are in the workspace.

For each crate we run:

  * `cargo rm a_crate_dep` (from cargo-edit)
  * `cargo check --all-targets`
  * `git reset --hard` (to get back to a clean state)

## Performance:

As an optimisation undepend skips dependencies that are clearly used in the source.
As such, the runtime of undepend is not too bad. (Before that optimisation it could
easily take overnight for some projects. Now I've not seen anything take longer than 30 mins for big
 projects on a 32 core box.)

## Gotchas:

At the moment `cargo check --all-targets` doesn't compile doc tests so if a dep is just used for that it might try and drop it.

(Tracking issues/PR for fixing this:
https://github.com/rust-lang/cargo/issues/6424
https://github.com/rust-lang/cargo/pull/8859
)

If the dependency is optional no attempt is made to removing it.

## Trophy Case:

Please reference this issue to add to the trophy case:

https://github.com/gilescope/undepend/issues/1

## Prior Art:

### udeps:

udeps takes a less brute-force approach of look at the incremental compile information in the target
dir to base its decisions on.

https://crates.io/crates/cargo-udeps

(Also compared to udeps this crate has only pure rust dependencies.)