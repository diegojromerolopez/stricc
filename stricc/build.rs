use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let workspace_dir = manifest_dir.parent().unwrap().to_path_buf();
    let runtime_dir = workspace_dir.join("runtime");

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let rt_target_dir = out_dir.join("rt-target");

    let target = env::var("TARGET").unwrap();
    let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());

    let mut cargo = Command::new("cargo");
    cargo
        .arg("build")
        .arg("--manifest-path")
        .arg(runtime_dir.join("Cargo.toml"))
        .arg("--target-dir")
        .arg(&rt_target_dir);

    // Pass the correct target architecture
    cargo.arg("--target").arg(&target);

    // Pass profile
    if profile == "release" {
        cargo.arg("--release");
    }

    // Run the build
    let status = cargo
        .status()
        .expect("failed to run cargo build for runtime library");
    if !status.success() {
        panic!("failed to build runtime library");
    }

    // The generated static library is located at:
    // rt-target/<target>/<profile>/libstricc_rt.a
    let mut rt_lib_path = rt_target_dir
        .join(&target)
        .join(if profile == "release" {
            "release"
        } else {
            "debug"
        })
        .join("libstricc_rt.a");

    if !rt_lib_path.exists() {
        rt_lib_path = rt_target_dir
            .join(if profile == "release" {
                "release"
            } else {
                "debug"
            })
            .join("libstricc_rt.a");
    }

    if !rt_lib_path.exists() {
        panic!("libstricc_rt.a not found at {rt_lib_path:?}");
    }

    // Copy the library to OUT_DIR so compiler can include_bytes! it
    let dest_path = out_dir.join("libstricc_rt.a");
    std::fs::copy(&rt_lib_path, &dest_path).unwrap();

    // Rerun build script if runtime source files or configs change
    println!(
        "cargo:rerun-if-changed={}",
        runtime_dir.join("src").join("lib.rs").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        runtime_dir.join("Cargo.toml").display()
    );
}
