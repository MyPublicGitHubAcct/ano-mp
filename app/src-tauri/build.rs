// Builds anomp_core (the C++ JUCE engine) with CMake and links it statically.
// The core is never a sidecar: iOS forbids helper processes (PLAN.md §1).

use std::path::PathBuf;

fn main() {
    build_core();
    tauri_build::build()
}

fn build_core() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let repo_root = manifest_dir.join("../..").canonicalize().unwrap();

    for path in ["CMakeLists.txt", "core"] {
        println!("cargo:rerun-if-changed={}", repo_root.join(path).display());
    }

    // The core has no install rules, so build just the library target and link
    // it from the build tree. The first build fetches JUCE (several minutes).
    let dst = cmake::Config::new(&repo_root)
        .generator("Ninja")
        .define("ANOMP_BUILD_TESTS", "OFF")
        .build_target("anomp_core")
        .build();

    println!("cargo:rustc-link-search=native={}", dst.join("build/core").display());
    println!("cargo:rustc-link-lib=static=anomp_core");

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    if target_os == "macos" {
        println!("cargo:rustc-link-lib=dylib=c++");
        for framework in [
            "Accelerate",
            "AppKit",
            "AudioToolbox",
            "AVFoundation",
            "CoreAudio",
            "CoreMIDI",
            "Foundation",
        ] {
            println!("cargo:rustc-link-lib=framework={framework}");
        }
    }
}
