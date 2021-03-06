use cargo_metadata::{Metadata, MetadataCommand};
use core::panic;
use std::path::{Path, PathBuf};
use std::process::Command;
use toml_edit::*;

#[derive(Debug)]
enum DependencyType {
    Normal,
    Dev,
    Build,
}

impl DependencyType {
    fn dep_group(&self) -> &'static str {
        match self {
            Self::Dev => "dev-dependencies",
            Self::Build => "build-dependencies",
            Self::Normal => "dependencies",
        }
    }

    /// name for things under package.metadata.cargo-udeps.ignore
    fn ignore_group(&self) -> &'static str {
        match self {
            Self::Dev => "development",
            Self::Build => "build",
            Self::Normal => "normal",
        }
    }

    fn extra_flag(&self) -> Option<&'static str> {
        match self {
            Self::Dev => Some("--dev"),
            Self::Build => Some("--build"),
            Self::Normal => None,
        }
    }
}

/// Ignore crates that we _know_ are only used with features turned on.
/// We are always going to *wrongly* remove them.
static GLOBAL_IGNORE : [&str; 1] = [
    "getrandom",  // Wasm js feature.
];

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

fn cargo_check(dir: &Path, check_doc_tests: bool, use_all_features: bool) -> Result<(), String> {
    let mut cmd = std::process::Command::new("cargo");
    cmd.args(vec!["check", "--all-targets"]);
    if use_all_features {
        cmd.arg("--all-features");
    }
    cmd.current_dir(dir);
    let result = cmd.status();
    let status = result.map_err(|e| e.to_string())?;
    if status.success() {
        cargo_build(dir, check_doc_tests, use_all_features)
    } else {
        Err(format!("{:?} failed", cmd))
    }
}

fn cargo_build(dir: &Path, check_doc_tests: bool, use_all_features: bool) -> Result<(), String> {
    println!("check: --release");
    let mut cmd = std::process::Command::new("cargo");
    cmd.args(vec!["build", "--all-targets"]);
    if use_all_features {
        cmd.arg("--all-features");
    }
    cmd.arg("--release");
    cmd.current_dir(dir);
    let result = cmd.status();
    let status = result.map_err(|e| e.to_string())?;
    if status.success() {
        if check_doc_tests {
            cargo_test(dir, use_all_features)
        } else {
            Ok(())
        }
    } else {
        Err(format!("{:?} failed", cmd))
    }
}

fn cargo_test(dir: &Path, use_all_features: bool) -> Result<(), String> {
    println!("last check: doc tests compile?");
    let mut cmd = std::process::Command::new("cargo");
    cmd.args(vec![
        "test",
        "--doc",
        "--release", // debug we only checked, but we've already build release so may be faster.
    ]);
    if use_all_features {
        cmd.arg("--all-features");
    }
    cmd.arg("bet_u_dont_have_a_test_called_this");
    cmd.arg("--");
    cmd.arg("--include-ignored");

    cmd.current_dir(dir);
    match cmd.output() {
        Ok(_output) => Ok(()),
        Err(err) => Err(format!("{:?} failed test compile due to {}", cmd, err)),
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
    check_doc_tests: bool,
    use_all_features: bool,
) -> Result<(), String> {
    for k in GLOBAL_IGNORE
    {
        if k == krate {
            return Err(format!("skipping {}\t[global ignore]", krate));
        }
    }

    cargo_rm(dep_type.extra_flag(), krate, dir)?;
    cargo_check(dir, check_doc_tests, use_all_features)?;

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

const VERSION: &'static str = env!("CARGO_PKG_VERSION");

/// Needs cargo edit, rg and git installed.
fn main() {
    if std::env::args_os().count() > 1 {
        println!("undepend v{}", VERSION);
        eprintln!("Run this in a clean checkout to reduce dependencies. No arguments needed.");
        println!("(to ignore some dependencies, write `package.metadata.cargo-udeps.ignore` in Cargo.toml)");
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

    let metadata = MetadataCommand::new()
        .manifest_path(repo_dir.join("Cargo.toml"))
        .exec()
        .unwrap();

    let use_all_features = cargo_check(&repo_dir, true, true).is_ok();

    println!("\nChecking for unused dependencies:");
    let mut doc_tests_exist = true;
    if let Err(msg) = cargo_check(&repo_dir, true, use_all_features) {
        if !msg.contains("no library targets found in package") {
            panic!("we need a clean build before we can proceed: {}", msg);
        }
        doc_tests_exist = false;
    }

    undepend(
        DependencyType::Normal,
        &metadata,
        &mut results,
        doc_tests_exist,
        use_all_features
    );
    println!("\nChecking for unused dev-dependencies:");
    undepend(
        DependencyType::Dev,
        &metadata,
        &mut results,
        doc_tests_exist,
        use_all_features
    );
    println!("\nChecking for unused build-dependencies:");
    undepend(
        DependencyType::Build,
        &metadata,
        &mut results,
        doc_tests_exist,
        use_all_features
    );
    println!("{}", results);
    if results.is_empty() {
        println!("???????????? no unused deps found ????????????");
    } else {
        std::fs::write("unused.sh", &results).unwrap();
        println!("Results written to unused.log");
    }
}

fn undepend(
    dep_type: DependencyType,
    metadata: &Metadata,
    mut results: &mut String,
    check_doc_tests: bool,
    use_all_features: bool
) {
    for package in metadata.workspace_members.iter() {
        let parts: Vec<_> = package.repr.split("path+file://").collect();
        let dir = PathBuf::from(&parts[1][..(parts[1].len() - 1)]);

        let file = std::fs::read_to_string(&dir.join("Cargo.toml")).unwrap();
        let toml_item = file.parse::<Document>().expect("invalid doc");

        if let Item::Table(table) = toml_item.root {
            if let Item::Table(ref deps) = table["package.metadata.cargo-udeps.ignore"] {
                println!("found an ignore table!! ");

                for (udep_type, v) in deps.iter() {
                    println!("found an ignore row!! ");
                    if dep_type.ignore_group() == udep_type {
                        println!("found a  {}", udep_type);
                        if let Item::Value(Value::Array(arr)) = v {
                            for krate in arr.iter() {
                                if let Value::String(krate_name) = krate {
                                    println!("found an ignore!! {}", krate_name);
                                }
                            }
                        }
                    }
                }
            }
            if let Item::Table(ref deps) = table[dep_type.dep_group()] {
                for (krate, v) in deps.iter() {
                    if let Item::Value(Value::InlineTable(tbl)) = v {
                        if let Some(Value::Boolean(val)) = tbl.get("optional") {
                            if *val.value() {
                                println!("skipping {}\t[optional]", krate);
                                continue;
                            }
                        }
                    }
                    if ripgrep(&dir, krate) {
                        println!("skipping {}\t[in use]", krate);
                        continue;
                    }

                    if let Err(msg) =
                        try_remove(&dep_type, krate, &dir, &mut results, check_doc_tests, use_all_features)
                    {
                        eprintln!("couldn't remove dependency {}: {}", krate, msg);
                    }
                    git_reset_hard(&dir);
                }
            }
        }
    }
}
