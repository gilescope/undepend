# undepend

undepend removes dependencies by brute force by removing likely candidates and checking that everything compiles after removal.

The results are a list of crates that can be removed in a file called `unused.sh`.
`chmod +x ./unused.sh` and run it to remove all the unused deps and see if the result works for you.

Alternatively you can `cat ./unused.sh` in a vscode terminal
and use the Cargo.toml hyperlinks to manually check that those crates ought to be removed.

## Installation:

(needs git)

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

## How undepend works:

cargo metadata is used to understand which crates are in the workspace.

For each crate we run:

  * `cargo rm a_crate_dep` (from cargo-edit)
  * `cargo check --all-targets`
  * `cargo build --all-targets --release`
  * `cargo test --doc --release non_existent_test_name`
  * If we get to here without errors then possibly the dep is not needed.
  * `git reset --hard` (to get back to a clean state)

## Performance:

As an optimisation undepend skips dependencies that are clearly used in the source.
As such, the runtime of undepend is not too bad. (Before that optimisation it could
easily take overnight for some projects. Now I've not seen anything take longer than 30 mins for big
 projects on a 32 core box.)

## Gotchas:

If a dependency is non-optional, is only used for one target platform and isn't directly used we may still try and remove it.

At the moment `cargo check --all-targets` doesn't compile doc tests so
we try and run `cargo test non_existent_test_name` - this I think forces the compile to happen.

(Tracking issues/PR for fixing this:
https://github.com/rust-lang/cargo/issues/6424
https://github.com/rust-lang/cargo/pull/8859
)

If the dependency is optional no attempt is made to removing it.

## Skipping:

TODO: We try and respect cargo-udeps ignore format:

`[package.metadata.cargo-udeps.ignore]`

(See [Cargo.toml](Cargo.toml) for an example)

## Trophy Case:

Please reference this issue to add to the trophy case:

https://github.com/gilescope/undepend/issues/1

## Prior Art:

### udeps:

udeps takes a less brute-force approach of look at the incremental compile information in the target
dir to base its decisions on and thus is better for regular CI use.

https://crates.io/crates/cargo-udeps

(Also compared to udeps this crate has only pure rust dependencies.)

## Changelog:

   * 0.1.1 Check debug + release mode and doc tests. Checking build dependencies also.
   * 0.1.0 Initial release