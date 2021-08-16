# undepend

Points to a fresh checkout and the tool will iterate over the workspace's dependencies, removing them individually and establishing if everything still compiles.

Records a list of dependencies that might be removable at ~/unused.log

# How this works:

cargo rm a_crate_dep
(from cargo-edit)

cargo check --all-targets

git reset --hard
