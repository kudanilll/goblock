//! Compiles GLSL shaders from `shaders/` to SPIR-V at build time.
//!
//! Vertex shaders live in `shaders/vertex/` and fragment shaders in
//! `shaders/fragment/`, all with the `.glsl` extension. Shared includes
//! (`common.glsl`, `scene.glsl`) sit in `shaders/` and are pulled in with
//! `#include "../common.glsl"`.
//!
//! Requires `glslc` from the Vulkan SDK. Set `VULKAN_SDK` or put `Bin` on
//! `PATH`. Output goes to `OUT_DIR/<name>.<stage>.spv`, embedded with
//! `include_bytes!`.
//!
//! Shaders target Vulkan 1.0, which emits SPIR-V 1.0. Vulkan 1.0 drivers
//! cannot load any newer SPIR-V version.

use std::path::{Path, PathBuf};
use std::process::Command;

fn find_glslc() -> Option<PathBuf> {
    let exe = if cfg!(windows) { "glslc.exe" } else { "glslc" };

    if let Ok(sdk) = std::env::var("VULKAN_SDK") {
        let candidate = Path::new(&sdk).join("Bin").join(exe);
        if candidate.exists() {
            return Some(candidate);
        }
        let candidate = Path::new(&sdk).join(exe);
        if candidate.exists() {
            return Some(candidate);
        }
    }

    // Common install locations on Windows, then PATH.
    #[cfg(windows)]
    {
        if let Some(home) = std::env::var_os("USERPROFILE") {
            for rel in [
                r"scoop\apps\vulkan\current\Bin",
                r"scoop\apps\vulkan\current",
                r"VulkanSDK",
            ] {
                let candidate = PathBuf::from(&home).join(rel).join(exe);
                if candidate.exists() {
                    return Some(candidate);
                }
            }
        }
        let base = Path::new(r"C:\VulkanSDK");
        if let Ok(entries) = std::fs::read_dir(base) {
            for entry in entries.flatten() {
                let candidate = entry.path().join("Bin").join(exe);
                if candidate.exists() {
                    return Some(candidate);
                }
            }
        }
    }

    // Fall back to PATH.
    Some(PathBuf::from(exe))
}

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let shader_dir = manifest_dir.join("../../shaders");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());

    let glslc = find_glslc().expect("glslc path");

    // (subdirectory, base name, stage). Water and the scene copy reuse the
    // block and sky vertex shaders.
    let shaders: &[(&str, &str, &str)] = &[
        ("vertex", "sky", "vert"),
        ("fragment", "sky", "frag"),
        ("vertex", "block", "vert"),
        ("fragment", "block", "frag"),
        ("vertex", "shadow", "vert"),
        ("fragment", "shadow", "frag"),
        ("vertex", "hud", "vert"),
        ("fragment", "hud", "frag"),
        ("fragment", "water", "frag"),
        ("fragment", "composite", "frag"),
        ("fragment", "background", "frag"),
    ];

    // Shared code that the shaders pull in with #include.
    for include in ["common.glsl", "scene.glsl"] {
        println!(
            "cargo:rerun-if-changed={}",
            shader_dir.join(include).display()
        );
    }

    for &(subdir, name, stage) in shaders {
        let input = shader_dir.join(subdir).join(format!("{name}.glsl"));
        let output = out_dir.join(format!("{name}.{stage}.spv"));

        println!("cargo:rerun-if-changed={}", input.display());

        let result = Command::new(&glslc)
            .arg("--target-env=vulkan1.0")
            .arg("-O")
            .arg(format!("-fshader-stage={stage}"))
            .arg("-I")
            .arg(&shader_dir)
            .arg(&input)
            .arg("-o")
            .arg(&output)
            .status()
            .unwrap_or_else(|e| {
                panic!(
                    "failed to run glslc at {}: {e}\nInstall the Vulkan SDK and set VULKAN_SDK.",
                    glslc.display()
                )
            });

        if !result.success() {
            panic!("glslc failed for {}", input.display());
        }
    }

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=VULKAN_SDK");
}
