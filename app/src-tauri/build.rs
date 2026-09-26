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
    link_ffmpeg(&repo_root, &target_os);

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

/// Links the shared FFmpeg built by scripts/build-ffmpeg.sh (LGPL requires it
/// to stay replaceable, so it is never linked statically).
fn link_ffmpeg(repo_root: &std::path::Path, target_os: &str) {
    let platform = match target_os {
        "macos" => "macos-universal",
        other => panic!("no FFmpeg build for {other} yet (PLAN.md Phases 8-10)"),
    };
    let lib_dir = repo_root.join("third_party/ffmpeg").join(platform).join("lib");
    assert!(
        lib_dir.exists(),
        "FFmpeg not found in {}; run scripts/build-ffmpeg.sh from the repository root",
        lib_dir.display()
    );

    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    for lib in ["avformat", "avcodec", "swresample", "avutil"] {
        println!("cargo:rustc-link-lib=dylib={lib}");
    }

    // The dylibs' install names are @rpath/...: find them in the build tree
    // during development. Bundling them into Contents/Frameworks (with an
    // @executable_path/../Frameworks rpath) is release work, PLAN.md §8.3.
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_dir.display());
}
