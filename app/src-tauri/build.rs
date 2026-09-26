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
    // TagLib (cmake/TagLib.cmake), a static dependency of the core.
    println!(
        "cargo:rustc-link-search=native={}",
        dst.join("build/_deps/taglib-build/taglib").display()
    );
    println!("cargo:rustc-link-lib=static=tag");

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    link_ffmpeg(&repo_root, &target_os);

    if target_os == "macos" {
        println!("cargo:rustc-link-lib=dylib=c++");
        link_clang_runtime();
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

/// JUCE's `@available` checks call `___isPlatformVersionAtLeast` from clang's
/// runtime, which rustc's link line leaves out. Debug builds pick up a weak
/// copy from Rust's std, but release LTO drops it as unused, so link clang's
/// (through xcrun, so it matches the active Xcode or Command Line Tools).
fn link_clang_runtime() {
    let output = std::process::Command::new("xcrun")
        .args(["clang", "-print-file-name=libclang_rt.osx.a"])
        .output()
        .expect("cannot run xcrun to find clang's runtime library");
    let runtime = String::from_utf8(output.stdout).unwrap().trim().to_owned();
    assert!(
        std::path::Path::new(&runtime).is_absolute(),
        "clang's runtime library not found (got {runtime:?})"
    );
    println!("cargo:rustc-link-arg={runtime}");
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

    // The dylibs' install names are @rpath/...: the app bundle carries them
    // in Contents/Frameworks (tauri.conf.json bundle.macOS.frameworks), and
    // tauri-build adds the @executable_path/../Frameworks rpath for that.
    // Debug builds (`tauri dev`, `cargo test`) also find them in the build
    // tree. Release binaries never search there: the sandbox refuses dylibs
    // outside the bundle, and a path under a protected folder such as
    // ~/Desktop makes macOS prompt for access. Release test runs get the
    // build tree from .cargo/config.toml instead.
    if std::env::var("PROFILE").as_deref() == Ok("debug") {
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_dir.display());
    }
}
