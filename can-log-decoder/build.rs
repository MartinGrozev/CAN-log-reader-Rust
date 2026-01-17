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

    let mut use_vcpkg_toolchain = false;
    let mut vcpkg_triplet: Option<String> = None;

    let profile = env::var("PROFILE").unwrap_or_else(|_| "release".to_string());
    let is_debug = profile.eq_ignore_ascii_case("debug");
    // On MSVC, always build mdflib with the release CRT to avoid /MDd vs /MD conflicts.
    let force_mdflib_release = cfg!(all(target_os = "windows", target_env = "msvc"));
    let mdflib_is_debug = is_debug && !force_mdflib_release;
    if force_mdflib_release && is_debug {
        println!("cargo:warning=Forcing mdflib Release build on MSVC to avoid CRT mismatch");
    }
    let cmake_profile = if mdflib_is_debug {
        "Debug"
    } else {
        "Release"
    };

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
        // Match Rust build profile to avoid CRT mismatches
        .profile(cmake_profile);

    // If vcpkg is available, use its toolchain for dependencies
    let vcpkg_root_opt = vcpkg_root.ok();

    if let Some(vcpkg_root) = vcpkg_root_opt.as_ref() {
        let toolchain = PathBuf::from(vcpkg_root)
            .join("scripts")
            .join("buildsystems")
            .join("vcpkg.cmake");

        if toolchain.exists() {
            println!("cargo:warning=Using vcpkg toolchain: {}", toolchain.display());
            cmake_config.define("CMAKE_TOOLCHAIN_FILE", toolchain.to_str().unwrap());
            let triplet = env::var("VCPKG_TARGET_TRIPLET")
                .unwrap_or_else(|_| "x64-windows-static-md".to_string());
            cmake_config.define("VCPKG_TARGET_TRIPLET", &triplet);
            vcpkg_triplet = Some(triplet);
            let installed_dir = PathBuf::from(vcpkg_root).join("installed");
            cmake_config.define("VCPKG_INSTALLED_DIR", installed_dir.to_str().unwrap());
            use_vcpkg_toolchain = true;
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
    println!("cargo:rustc-link-search=native={}/build/mdflib/Debug", dst.display());
    let mdflib_name = if mdflib_is_debug { "mdfd" } else { "mdf" };
    println!("cargo:rustc-link-lib=static={}", mdflib_name);

    // On Windows with MSVC, we may need additional system libraries
    #[cfg(target_os = "windows")]
    {
        // Windows system libraries that mdflib might need
        println!("cargo:rustc-link-lib=dylib=ws2_32");
        println!("cargo:rustc-link-lib=dylib=advapi32");
        println!("cargo:rustc-link-lib=dylib=userenv");

        if use_vcpkg_toolchain {
            if let (Some(vcpkg_root), Some(triplet)) = (vcpkg_root_opt.as_ref(), vcpkg_triplet.as_ref()) {
                let installed_triplet = PathBuf::from(vcpkg_root).join("installed").join(triplet);
                println!("cargo:rustc-link-search=native={}/lib", installed_triplet.display());
            }

            // Prefer static linking for vcpkg dependencies to avoid shipping DLLs.
            println!("cargo:rustc-link-lib=static=zlib");
            let expat_name = if vcpkg_triplet
                .as_ref()
                .map(|t| t.ends_with("static-md"))
                .unwrap_or(false)
            {
                "libexpatMD"
            } else {
                "libexpat"
            };
            println!("cargo:rustc-link-lib=static={}", expat_name);
        }
    }

    // On Linux, link against standard C++ library
    #[cfg(target_os = "linux")]
    {
        println!("cargo:rustc-link-lib=dylib=stdc++");
    }

    // Build our C API wrapper that bridges Rust and mdflib C++
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    let mut cxx_build = cc::Build::new();

    #[cfg(all(target_os = "windows", target_env = "msvc"))]
    {
        if mdflib_is_debug {
            cxx_build.flag("/MDd");
        } else {
            cxx_build.flag("/MD");
        }
    }

    cxx_build
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
