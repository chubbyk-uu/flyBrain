use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=rust/cuda/flybrain_cuda.cu");
    println!("cargo:rerun-if-env-changed=CUDA_HOME");
    println!("cargo:rerun-if-env-changed=FLYBRAIN_CUDA_ARCH");

    if env::var_os("CARGO_FEATURE_CUDA").is_none() {
        return;
    }
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("linux") {
        panic!("the cuda feature currently supports Linux/WSL only");
    }

    let cuda_home =
        PathBuf::from(env::var_os("CUDA_HOME").unwrap_or_else(|| "/usr/local/cuda".into()));
    let nvcc = cuda_home.join("bin/nvcc");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo did not set OUT_DIR"))
        .join("libflybrain_cuda.a");
    let arch = env::var("FLYBRAIN_CUDA_ARCH").unwrap_or_else(|_| "sm_120".to_owned());
    let status = Command::new(&nvcc)
        .args([
            "--lib",
            "-O3",
            "--std=c++17",
            "-Xcompiler",
            "-fPIC",
            "-arch",
            &arch,
            "rust/cuda/flybrain_cuda.cu",
            "-o",
        ])
        .arg(&output)
        .status()
        .unwrap_or_else(|error| panic!("failed to run {}: {error}", nvcc.display()));
    assert!(status.success(), "nvcc failed with status {status}");

    println!(
        "cargo:rustc-link-search=native={}",
        output.parent().unwrap().display()
    );
    println!(
        "cargo:rustc-link-search=native={}",
        cuda_home.join("lib64").display()
    );
    println!("cargo:rustc-link-lib=static=flybrain_cuda");
    println!("cargo:rustc-link-lib=dylib=cudart");
    println!("cargo:rustc-link-lib=dylib=stdc++");
}
