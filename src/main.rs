use cargo_metadata::MetadataCommand;
use std::path::{Path, PathBuf};
use std::process::Command;
use toml_edit::*;

#[derive(Debug)]
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

fn ripgrep(dir: &Path, needle: &str) -> bool {
    // Note: This is overly cautious as if there's a subdir with a crate in it that does use this dependency it will
    // also assume it's used.
    Command::new("rg")
        .args(&["--type", "rust"])
        .arg("-q") // -w was not good enough here. It failed to spot some usages.
        .arg(format!("{}::", needle.replace("-", "_")))
        .arg(&dir)
        .status()
        .expect("rg not found. Solution: cargo install ripgrep")
        .success()
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

fn check_cargo_edit_installed() -> Result<(), String> {
    let mut cmd = std::process::Command::new("cargo");
    cmd.stdout(std::process::Stdio::null());
    cmd.arg("rm");
    cmd.arg("--help");
    let result = cmd.status().map_err(|e| e.to_string())?;
    if result.success() {
        Ok(())
    } else {
        Err(format!("cargo edit not installed."))
    }
}

fn git_reset_hard(dir: &Path) {
    let mut cmd = std::process::Command::new("git");
    cmd.args(vec!["reset", "--hard"]);
    cmd.current_dir(dir);
    cmd.status().expect("Panic: git reset --hard doesn't work.");
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
    results: &mut String,
) -> Result<(), String> {
    cargo_rm(dep_type.extra_flag(), krate, dir)?;
    cargo_check(dir)?;

    results.push_str(&format!(
        "\n# {}/Cargo.toml {} {:?}\n(cd {} && cargo rm {} {})",
        &dir.to_string_lossy(),
        krate,
        dep_type,
        dir.to_string_lossy(),
        dep_type.extra_flag().unwrap_or(""),
        krate
    ));
    Ok(())
}

/// Needs cargo edit, rg and git installed.
fn main() {
    if std::env::args_os().count() > 1 {
        eprintln!("run this in a clean checkout to reduce dependencies. No arguments needed.");
        return;
    }
    if check_cargo_edit_installed().is_err() {
        eprintln!("Please cargo install cargo-edit");
        return;
    }
    let repo_dir = PathBuf::from(
        std::env::current_dir()
            .expect("no current dir and --repo not specified")
            .to_string_lossy()
            .to_string(),
    );
    bail_if_checkout_dirty(&repo_dir);
    // let home = dirs::home_dir().expect("home dir not found");
    // let result_file = home.join("unused.log");
    let mut results = String::new();
    undepend(DependencyType::Main, repo_dir.clone(), &mut results);
    undepend(DependencyType::Dev, repo_dir, &mut results);
    println!("{}", results);
    std::fs::write("unused.sh", &results).unwrap();
}

fn undepend(dep_type: DependencyType, repo_dir: PathBuf, mut results: &mut String) {
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
                for (krate, v) in deps.iter() {
                    if let Item::Value(Value::InlineTable(tbl)) = v {
                        if let Some(Value::Boolean(val)) = tbl.get("optional") {
                            if *val.value() {
                                println!("dep is optional {} - skipping", krate);
                                continue;
                            }
                        }
                    }
                    if ripgrep(&dir, krate) {
                        println!("looks like {} is used - skipping", krate);
                        continue;
                    }

                    if let Err(msg) = try_remove(&dep_type, krate, &dir, &mut results) {
                        eprintln!("couldn't remove dependency {}: {}", krate, msg);
                    }
                    git_reset_hard(&dir);
                }
            }
        }
    }
    println!("Results written to unused.log");
}
