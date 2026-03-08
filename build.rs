// build.rs – KeraDB Rust SDK
//
// Emits Cargo link-search hints so the native keradb shared library can be
// found at compile/link time. The SDK uses `libloading` for runtime loading,
// so the crate itself compiles without the library, but users need it at
// runtime. This script prints a clear actionable error if the library is
// absent during a build that would require it.

use std::env;
use std::path::PathBuf;

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    let lib_name = match target_os.as_str() {
        "windows" => "keradb.dll",
        "macos" => "libkeradb.dylib",
        _ => "libkeradb.so",
    };

    // Standard search paths, highest priority first.
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let workspace_release = manifest_dir
        .ancestors()
        .nth(3) // sdks/rust -> sdks -> workspace-root
        .map(|p| p.join("target").join("release"))
        .unwrap_or_default();

    let search_paths = vec![
        manifest_dir.clone(),
        workspace_release.clone(),
        PathBuf::from("/usr/local/lib"),
        PathBuf::from("/usr/lib"),
    ];

    // Emit link-search hints for every candidate path.
    for path in &search_paths {
        if path.exists() {
            println!("cargo:rustc-link-search=native={}", path.display());
        }
    }

    // If the library is nowhere to be found, print a helpful message.
    // (This is advisory only – the crate compiles either way because
    //  libloading does runtime loading, not compile-time linking.)
    let found = search_paths.iter().any(|p| p.join(lib_name).exists());
    if !found {
        println!(
            "cargo:warning=\
KeraDB native library ({lib_name}) was not found in any standard search path.\n\
The SDK will compile but will return a LibraryLoad error at runtime.\n\
Build the native library first:\n\
  - From source:  cd keradb && cargo build --release\n\
  - Pre-built:    https://github.com/keradb/keradb/releases\n\
Then place {lib_name} in one of:\n\
  {paths}",
            paths = search_paths
                .iter()
                .map(|p| format!("  • {}", p.display()))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    // Re-run only when the build script itself changes.
    println!("cargo:rerun-if-changed=build.rs");
}
