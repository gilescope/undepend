# undepend

Points to a fresh checkout and the tool will iterate over the workspace's dependencies,
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
