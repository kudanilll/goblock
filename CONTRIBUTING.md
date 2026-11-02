# Contributing to GoBlock

Thank you for your interest in contributing to GoBlock. This guide explains how to set up the development environment, run tests, and follow the project's architecture rules.

## Prerequisites

Building GoBlock requires:

- Rust 1.85 or newer. The workspace MSRV is 1.85.
- The Vulkan SDK with `glslc` available on `PATH` or in `VULKAN_SDK`. On Windows, the build script also checks common paths under `C:\VulkanSDK` and scoop.
- A GPU with Vulkan 1.0 support and an up-to-date driver.

On Linux, install the Vulkan loader and development headers through your package manager (such as `libvulkan-dev` or `vulkan-headers`).

## Building and running

Check that the project compiles:

```bash
cargo build
```

Run the game in release mode:

```bash
cargo run --release
```

Run the chunk generation and meshing benchmark:

```bash
cargo run --release -p goblock-world --example stream_stats
```

## Verification before submitting

Before opening a pull request, run these four checks locally:

```bash
cargo test --workspace
cargo clippy --workspace --all-targets
cargo fmt --all --check
cargo +1.85 check --workspace --all-targets
```

The workspace tests run without a GPU because `core`, `world`, and `input` do not link against Vulkan or windowing libraries. All unit tests must pass, clippy must report zero warnings, and formatting must match `rustfmt`.

When fixing a bug or adding logic in `core`, `world`, `input`, or `app`, write a failing test first. Verify that the test fails for the expected reason, then make the change and confirm it passes.

## Architecture rules

GoBlock follows strict architectural boundaries. Pull requests should follow these rules:

### Vulkan 1.0 core

- The Vulkan instance requests `API_VERSION_1_0`. Do not call Vulkan 1.1, 1.2, or 1.3 functions.
- The only enabled device extension is `VK_KHR_swapchain`. Any other extension requires an explicit runtime probe and a graceful fallback.
- Shaders compile to SPIR-V 1.0 with `--target-env=vulkan1.0`. Do not use extensions that require newer SPIR-V versions.
- Pipeline push constants must fit within 128 bytes. Larger per-frame data belongs in the `Frame` uniform block in `shaders/common.glsl`.

### Crate boundaries

Dependencies point one way through the workspace:

- `core`: generic math, frustum culling, and utilities. No external game dependencies.
- `world`: block definitions, procedural terrain, biome classification, meshing, and chunk streaming.
- `input`: camera orientation, player collision, and input traits.
- `render`: Vulkan pipelines, swapchain, render passes, texture upload, and font atlas.
- `app`: window management, event loop, menus, settings, and world saves.

`core`, `world`, and `input` must remain free of `ash` and `winit`. Only `render` touches Vulkan, and only `app` touches `winit` and runtime filesystem operations.

### Shader structure

Shaders live in `shaders/`:

- Vertex shaders belong in `shaders/vertex/` with the `.glsl` extension.
- Fragment shaders belong in `shaders/fragment/` with the `.glsl` extension.
- Shared includes live at the root of `shaders/` (`common.glsl`, `scene.glsl`).

The build script in `crates/render/build.rs` compiles each shader to SPIR-V at build time.

### Compatibility

- Rust 1.85 compatibility is required. Do not use standard library features stabilised in Rust 1.86 or later.
- The project uses the 2021 edition. Let chains are not available.
- Code comments should be plain sentences without promotional phrasing or conversational filler.

### Assets and third-party code

- Block textures are 16x16 PNGs in `assets/textures/blocks/`.
- Menu background slides live in `assets/backgrounds/`.
- Fonts live in `assets/fonts/`.
- Any imported third-party code, asset, or font must be credited in `THIRD-PARTY-NOTICES.md` with its license text placed in `licenses/`.
- Never copy code or assets from the untracked `reference/` or `build/` directories.

## Pull requests

1. Create a feature branch from `master`.
2. Keep the scope of the pull request focused on a single feature or bug fix.
3. Write clear commit messages that describe what changed and why.
4. Fill out the pull request template and check off each verification step.
