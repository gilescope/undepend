# undepend

## installation

**assumes you have git**
cargo install ripgrep cargo-edit undepend

Point to a fresh clone and the tool will iterate over the workspace's dependencies,
removing them individually and establishing if everything still compiles.

Records a list of dependencies that might be removable as it goes at ~/unused.log
(feel free to tail the file)

`cargo install undepend --path .`

Then in your _clean_ rust project that you've checked out run:
`undepend`

## How this works:

cargo rm a_crate_dep
(from cargo-edit)

cargo check --all-targets

git reset --hard

## Gotchas:

At the moment cargo check --all-targets doesn't compile doc tests so if a dep is just used for that it might try and drop it.

(Tracking issues/PR for fixing this:
https://github.com/rust-lang/cargo/issues/6424
https://github.com/rust-lang/cargo/pull/8859
)

If the dependency is optional no attempt is made to removing it.

## Trophy Case:

Please reference this issue to add to the trophy case:
https://github.com/gilescope/undepend/issues/1