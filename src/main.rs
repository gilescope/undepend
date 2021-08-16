use argh::FromArgs;
use cargo_metadata::MetadataCommand;
use std::fs::OpenOptions;
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use std::process::Command;
use toml_edit::*;

#[allow(dead_code)]
enum DependencyType {
    Main,
    Dev,
}

impl DependencyType {
    fn dep_group(&self) -> &'static str {
        match self {
            Self::Dev => "dev-dependencies",
            Self::Main => "dependencies",
        }
    }
    fn extra_flag(&self) -> Option<&'static str> {
        match self {
            Self::Dev => Some("--dev"),
            Self::Main => None,
        }
    }
}

#[derive(FromArgs)]
#[argh(description = "Take away deps and see if it builds")]
struct Config {
    #[argh(
        switch,
        short = 'd',
        description = "instead of normal deps, reduce dev dependencies"
    )]
    dev_dependencies: bool,

    #[argh(
        option,
        default = "String::from(\"/home/gilescope/git/polkadot4\")",
        description = "clean git checkout location"
    )]
    repo: String,
}

fn ripgrep(dir: &Path, needle: &str) -> bool {
    // Note: This is overly cautious as if there's a subdir with a crate in it that does use this dependency it will
    // also assume it's used.
    Command::new("rg")
        .args(&["--type", "rust"])
        .arg("-qw")
        .arg(format!("{}::", needle.replace("-", "_")))
        .arg(&dir)
        .status()
        .unwrap()
        .success()
}

/// Record a result outside of the git repo
fn append_line_to_log(result_file: &Path, dir: &Path, krate: &str) {
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .append(true)
        .open(result_file)
        .unwrap();
    if let Err(e) = writeln!(
        file,
        "WORKED: {}/Cargo.toml\t{}",
        dir.to_string_lossy(),
        krate
    ) {
        eprintln!("Couldn't write file: {}", e);
    }
}

fn cargo_check(dir: &Path) -> Result<(), String> {
    let mut cmd = std::process::Command::new("cargo");
    cmd.args(vec!["check", "--all-targets"]);
    cmd.current_dir(dir);
    let result = cmd.status();
    let status = result.map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{:?} failed", cmd))
    }
}

fn cargo_rm(rm_flag: Option<&str>, k: &str, dir: &Path) -> Result<(), String> {
    let mut cmd = std::process::Command::new("cargo");
    cmd.arg("rm");
    if let Some(rm_flag) = rm_flag {
        cmd.arg(rm_flag);
    }
    cmd.arg(k);
    cmd.current_dir(dir);
    let result = cmd.status().map_err(|e| e.to_string())?;
    if result.success() {
        Ok(())
    } else {
        Err(format!(
            "couldn't cargo rm dependency..{:?} {:?}",
            &dir, cmd
        ))
    }
}

fn git_reset_hard(dir: &Path) {
    let mut cmd = std::process::Command::new("git");
    cmd.args(vec!["reset", "--hard"]);
    cmd.current_dir(dir);
    cmd.status()
        .expect("Panic: If git reset doesn't work we can't go on.");
}

fn bail_if_checkout_dirty(repo_dir: &Path) {
    let mut cmd = std::process::Command::new("git");
    cmd.args(vec!["status", "--porcelain"]);
    cmd.current_dir(repo_dir);
    let out = cmd
        .output()
        .expect("Panic: Could not determine if git clone was clean");
    if !out.stdout.is_empty() || !out.stderr.is_empty() {
        eprintln!("Repository is not clean. This tool will only work on a fresh checkout.");
        std::process::exit(-1);
    }
}

/// This leaves the checkout altered. needs a git reset after.
fn try_remove(
    dep_type: &DependencyType,
    krate: &str,
    dir: &Path,
    result_file: &Path,
) -> Result<(), String> {
    cargo_rm(dep_type.extra_flag(), krate, dir)?;
    cargo_check(dir)?;

    println!("WORKED:{:?} {} {:?}", dep_type.extra_flag(), krate, &dir);
    append_line_to_log(result_file, dir, krate);
    Ok(())
}

/// Needs cargo edit, rg and git installed.
fn main() {
    let config: Config = argh::from_env();
    let home = dirs::home_dir().expect("home dir not found");
    let repo_dir = PathBuf::from(config.repo);
    let dep_type = if config.dev_dependencies {
        DependencyType::Dev
    } else {
        DependencyType::Main
    };

    bail_if_checkout_dirty(&repo_dir);

    let result_file = home.join("unused.log");

    let metadata = MetadataCommand::new()
        .manifest_path(repo_dir.join("Cargo.toml"))
        .exec()
        .unwrap();

    for (i, p) in metadata.workspace_members.iter().enumerate() {
        let parts: Vec<_> = p.repr.split("path+file://").collect();
        let dir = PathBuf::from(&parts[1][..(parts[1].len() - 1)]);
        println!("processing {}: {:?}", i, &dir);

        let file = std::fs::read_to_string(&dir.join("Cargo.toml")).unwrap();
        let toml_item = file.parse::<Document>().expect("invalid doc");

        if let Item::Table(table) = toml_item.root {
            if let Item::Table(ref deps) = table[dep_type.dep_group()] {
                for (krate, _v) in deps.iter() {
                    if ripgrep(&dir, krate) {
                        println!("looks like {} is used - skipping", krate);
                        continue;
                    }

                    if let Err(msg) = try_remove(&dep_type, krate, &dir, &result_file) {
                        eprintln!("couldn't remove dependency {}: {}", krate, msg);
                    }
                    git_reset_hard(&dir);
                }
            }
        }
    }
}
