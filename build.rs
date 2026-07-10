use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=native/audio_capture.m");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is set by Cargo"));
    let object = out_dir.join("audio_capture.o");
    let library = out_dir.join("libaudio_capture.a");
    let architecture = match env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("aarch64") => "arm64",
        Ok("x86_64") => "x86_64",
        Ok(other) => panic!("unsupported macOS architecture for audio capture: {other}"),
        Err(_) => panic!("Cargo did not provide CARGO_CFG_TARGET_ARCH"),
    };

    run(
        Command::new("xcrun")
            .args([
                "--sdk",
                "macosx",
                "clang",
                "-fobjc-arc",
                "-std=gnu11",
                "-mmacosx-version-min=11.0",
                "-arch",
                architecture,
                "-c",
                "native/audio_capture.m",
                "-o",
            ])
            .arg(&object),
        "compile native system-audio capture bridge",
    );

    run(
        Command::new("xcrun")
            .args(["libtool", "-static", "-o"])
            .arg(&library)
            .arg(&object),
        "archive native system-audio capture bridge",
    );

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=audio_capture");
    let clang_runtime = clang_resource_directory().join("lib/darwin");
    assert!(
        clang_runtime.join("libclang_rt.osx.a").is_file(),
        "could not find libclang_rt.osx.a in {}",
        clang_runtime.display()
    );
    println!("cargo:rustc-link-search=native={}", clang_runtime.display());
    println!("cargo:rustc-link-lib=static=clang_rt.osx");
    println!("cargo:rustc-link-lib=framework=CoreAudio");
    println!("cargo:rustc-link-lib=framework=Foundation");
    println!("cargo:rustc-link-lib=objc");
}

fn clang_resource_directory() -> PathBuf {
    let output = Command::new("xcrun")
        .args(["--sdk", "macosx", "clang", "-print-resource-dir"])
        .output()
        .expect("failed to locate the Clang resource directory");
    assert!(
        output.status.success(),
        "failed to locate the Clang resource directory: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    PathBuf::from(
        String::from_utf8(output.stdout)
            .expect("Clang resource directory is UTF-8")
            .trim(),
    )
}

fn run(command: &mut Command, action: &str) {
    let status = command
        .status()
        .unwrap_or_else(|error| panic!("failed to {action}: {error}"));
    assert!(status.success(), "failed to {action}: {status}");
}
