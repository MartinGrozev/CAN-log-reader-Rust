//! Build script for can-log-decoder
//!
//! This script compiles the mdflib C++ library using CMake and links it
//! into the Rust binary for MF4 file parsing support.

use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../vendor/mdflib");

    // Get the workspace root (parent of can-log-decoder)
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let workspace_root = manifest_dir.parent().unwrap();
    let mdflib_src = workspace_root.join("vendor").join("mdflib");

    println!("cargo:warning=Building mdflib from: {}", mdflib_src.display());

    // Check if mdflib source exists
    if !mdflib_src.exists() {
        println!("cargo:warning=mdflib source not found at {}", mdflib_src.display());
        println!("cargo:warning=MF4 parser will not be available");
        return;
    }

    // Check for vcpkg installation (for ZLIB and EXPAT dependencies)
    let vcpkg_root = env::var("VCPKG_ROOT")
        .or_else(|_| {
            // Try default vcpkg location
            let home = env::var("USERPROFILE").or_else(|_| env::var("HOME"))?;
            let vcpkg_path = PathBuf::from(home).join("vcpkg");
            if vcpkg_path.exists() {
                Ok(vcpkg_path.to_string_lossy().to_string())
            } else {
                Err(env::VarError::NotPresent)
            }
        });

    // Configure CMake to build mdflib as a static library
    let mut cmake_config = cmake::Config::new(&mdflib_src);

    cmake_config
        // Build only the core mdflib library (no tools, tests, etc.)
        .define("MDF_BUILD_SHARED_LIB", "OFF")
        .define("MDF_BUILD_SHARED_LIB_NET", "OFF")
        .define("MDF_BUILD_SHARED_LIB_EXAMPLE", "OFF")
        .define("MDF_BUILD_DOC", "OFF")
        .define("MDF_BUILD_TOOL", "OFF")
        .define("MDF_BUILD_TEST", "OFF")
        .define("MDF_BUILD_PYTHON", "OFF")
        .define("BUILD_SHARED_LIBS", "OFF")
        // Use static MSVC runtime for better portability
        .define("CMAKE_MSVC_RUNTIME_LIBRARY", "MultiThreaded$<$<CONFIG:Debug>:Debug>")
        // Build in Release mode for better performance
        .profile("Release");

    // If vcpkg is available, use its toolchain for dependencies
    if let Ok(vcpkg_root) = vcpkg_root {
        let toolchain = PathBuf::from(&vcpkg_root)
            .join("scripts")
            .join("buildsystems")
            .join("vcpkg.cmake");

        if toolchain.exists() {
            println!("cargo:warning=Using vcpkg toolchain: {}", toolchain.display());
            cmake_config.define("CMAKE_TOOLCHAIN_FILE", toolchain.to_str().unwrap());
            cmake_config.define("VCPKG_TARGET_TRIPLET", "x64-windows-static");
        } else {
            println!("cargo:warning=vcpkg found but toolchain not available at: {}", toolchain.display());
        }
    } else {
        println!("cargo:warning=vcpkg not found - ZLIB and EXPAT must be installed separately");
    }

    let dst = cmake_config.build();

    println!("cargo:warning=mdflib built successfully at: {}", dst.display());

    // Link against the built mdflib library
    // Add multiple search paths since CMake puts libs in different places
    println!("cargo:rustc-link-search=native={}/lib", dst.display());
    println!("cargo:rustc-link-search=native={}/mdf/lib", dst.display());
    println!("cargo:rustc-link-search=native={}/build/mdflib/Release", dst.display());
    println!("cargo:rustc-link-lib=static=mdf");

    // On Windows with MSVC, we may need additional system libraries
    #[cfg(target_os = "windows")]
    {
        // Windows system libraries that mdflib might need
        println!("cargo:rustc-link-lib=dylib=ws2_32");
        println!("cargo:rustc-link-lib=dylib=advapi32");
        println!("cargo:rustc-link-lib=dylib=userenv");
    }

    // On Linux, link against standard C++ library
    #[cfg(target_os = "linux")]
    {
        println!("cargo:rustc-link-lib=dylib=stdc++");
    }

    // Build our C API wrapper that bridges Rust and mdflib C++
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    cc::Build::new()
        .cpp(true)
        .file(mdflib_src.join("mdf_c_api.cpp"))
        .include(mdflib_src.join("include"))
        .include(&dst.join("include"))  // mdflib headers
        .flag_if_supported("/std:c++17")  // MSVC
        .flag_if_supported("-std=c++17")  // GCC/Clang
        .compile("mdf_c_api");

    println!("cargo:warning=C API wrapper built successfully");
    println!("cargo:warning=mdflib linking configured successfully");
}
