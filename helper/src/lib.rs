use chrono::Local;
#[cfg(feature = "clap")]
use clap::Parser;
use std::{
    io::{BufRead, BufReader},
    process::{Command, Stdio},
    thread,
};

#[derive(Default, Clone)]
// `clap` is enabled in the `cli` crate for `sp1-helper`, when users are building programs with
// `cargo prove` CLI directly. The `helper` crate is intended to be lightweight, so we only derive
// the `Parser` trait if the `clap` feature is enabled.
#[cfg_attr(feature = "clap", derive(Parser))]
pub struct BuildArgs {
    #[cfg_attr(
        feature = "clap",
        clap(long, action, help = "Build using Docker for reproducible builds.")
    )]
    pub docker: bool,
    #[cfg_attr(
        feature = "clap",
        clap(long, action, help = "Ignore Rust version check.")
    )]
    pub ignore_rust_version: bool,
    #[cfg_attr(
        feature = "clap",
        clap(long, action, help = "If building a binary, specify the name.")
    )]
    pub binary: Option<String>,
    #[cfg_attr(feature = "clap", clap(long, action, help = "ELF binary name."))]
    pub elf: Option<String>,
    #[cfg_attr(feature = "clap", clap(long, action, help = "Build with features."))]
    pub features: Vec<String>,
}

fn current_datetime() -> String {
    let now = Local::now();
    now.format("%Y-%m-%d %H:%M:%S").to_string()
}

/// Re-run the cargo command if the Cargo.toml or Cargo.lock file changes.
pub fn cargo_rerun_if_changed(path: &str) {
    println!("path: {:?}", path);
    let program_dir = std::path::Path::new(path);

    // Tell cargo to rerun the script if program/{src, Cargo.toml, Cargo.lock} or any dependency
    // changes.
    let metadata_file = program_dir.join("Cargo.toml");
    let mut metadata_cmd = cargo_metadata::MetadataCommand::new();
    let metadata = metadata_cmd.manifest_path(metadata_file).exec().unwrap();
    println!(
        "cargo:rerun-if-changed={}",
        metadata.workspace_root.join("Cargo.lock").as_str()
    );

    for package in &metadata.packages {
        println!("cargo:rerun-if-changed={}", package.manifest_path.as_str());
    }

    // Print a message so the user knows that their program was built. Cargo caches warnings emitted
    // from build scripts, so we'll print the date/time when the program was built.
    let root_package = metadata.root_package();
    let root_package_name = root_package
        .as_ref()
        .map(|p| p.name.as_str())
        .unwrap_or("Program");
    println!(
        "cargo:warning={} built at {}",
        root_package_name,
        current_datetime()
    );
}

/// Builds the program if the program at path, or one of its dependencies, changes.
/// Note: This function is kept for backwards compatibility.
pub fn build_program(path: &str) {
    // Activate the build command if the dependencies change.
    cargo_rerun_if_changed(path);

    let status = execute_build_cmd(&program_dir, None)
        .unwrap_or_else(|_| panic!("Failed to build `{}`.", root_package_name));
    if !status.success() {
        panic!("Failed to build `{}`.", root_package_name);
    }
}

/// Builds the program with the given arguments if the program at path, or one of its dependencies,
/// changes.
pub fn build_program_with_args(path: &str, args: BuildArgs) {
    // Activate the build command if the dependencies change.
    cargo_rerun_if_changed(path);

    let status = execute_build_cmd(&program_dir, Some(args))
        .unwrap_or_else(|_| panic!("Failed to build `{}`.", root_package_name));
    if !status.success() {
        panic!("Failed to build `{}`.", root_package_name);
    }
}

/// Add the `cargo prove build` arguments to the `command_args` vec. This is useful when adding
/// the `cargo prove build` arguments to an existing command.
pub fn add_cargo_prove_build_args(
    command_args: &mut Vec<String>,
    prove_args: BuildArgs,
    ignore_docker: bool,
) {
    if prove_args.docker && !ignore_docker {
        command_args.push("--docker".to_string());
    }
    if prove_args.ignore_rust_version {
        command_args.push("--ignore-rust-version".to_string());
    }
    if !prove_args.features.is_empty() {
        for feature in prove_args.features {
            command_args.push("--features".to_string());
            command_args.push(feature);
        }
    }
    if let Some(binary) = &prove_args.binary {
        command_args.push("--binary".to_string());
        command_args.push(binary.clone());
    }
    if let Some(elf) = &prove_args.elf {
        command_args.push("--elf".to_string());
        command_args.push(elf.clone());
    }
}

/// Executes the `cargo prove build` command in the program directory
fn execute_build_cmd(
    program_dir: &impl AsRef<std::path::Path>,
    args: Option<BuildArgs>,
) -> Result<std::process::ExitStatus, std::io::Error> {
    // Check if RUSTC_WORKSPACE_WRAPPER is set to clippy-driver (i.e. if `cargo clippy` is the current
    // compiler). If so, don't execute `cargo prove build` because it breaks rust-analyzer's `cargo clippy` feature.
    let is_clippy_driver = std::env::var("RUSTC_WORKSPACE_WRAPPER")
        .map(|val| val.contains("clippy-driver"))
        .unwrap_or(false);
    if is_clippy_driver {
        println!("cargo:warning=Skipping build due to clippy invocation.");
        return Ok(std::process::ExitStatus::default());
    }

    let mut cargo_prove_build_args = vec!["prove".to_string(), "build".to_string()];
    /// Add the arguments for the `cargo prove build` CLI to the command.
    if let Some(args) = args {
        add_cargo_prove_build_args(&mut cargo_prove_build_args, args, is_clippy_driver);
    }

    let mut cmd = Command::new("cargo");
    cmd.current_dir(program_dir)
        .args(cargo_prove_build_args)
        .env_remove("RUSTC")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn()?;

    let stdout = BufReader::new(child.stdout.take().unwrap());
    let stderr = BufReader::new(child.stderr.take().unwrap());

    // Pipe stdout and stderr to the parent process with [sp1] prefix
    let stdout_handle = thread::spawn(move || {
        stdout.lines().for_each(|line| {
            println!("[sp1] {}", line.unwrap());
        });
    });
    stderr.lines().for_each(|line| {
        eprintln!("[sp1] {}", line.unwrap());
    });

    stdout_handle.join().unwrap();

    child.wait()
}
