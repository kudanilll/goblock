# GoBlock

A block themed casual game written in Rust, rendered with Vulkan through `ash`.

The world is a grid of 8x8 chunks with simple height variation. The camera flies with WASD and mouse look, and the debug HUD shows position and FPS in the top-left corner.

## Requirements

- Rust 1.85 or newer
- A GPU and driver with Vulkan 1.0 or newer
- The Vulkan SDK, for `glslc` and validation layers

On Windows with [scoop](https://scoop.sh):

```powershell
scoop install vulkan
```

On Debian and Ubuntu:

```sh
sudo apt install vulkan-tools libvulkan-dev glslang-tools
```

If `glslc` is not on `PATH`, the build script looks for it under `VULKAN_SDK`. Set that variable if your SDK lives somewhere unusual.

## Running

```sh
cargo run --release
```

The first build compiles the Vulkan stack and every shader to SPIR-V, so it takes a few minutes.

### Controls

| Input           | Action      |
| --------------- | ----------- |
| `W` `A` `S` `D` | Move        |
| Mouse           | Look around |
| `Shift`         | Sprint      |
| `Space`         | Fly up      |
| `Ctrl` or `C`   | Fly down    |
| `Esc`           | Quit        |

## Shaders

GLSL lives in `shaders/vulkan/` and is compiled by `crates/render/build.rs` into `${OUT_DIR}` with `glslc`, then embedded in the binary. The shaders target Vulkan 1.0, so the output is SPIR-V 1.0. Debug builds turn on the validation layer when it is installed and print its messages to stderr.

## Hardware support

The renderer uses Vulkan 1.0 core plus `VK_KHR_surface` and `VK_KHR_swapchain`, and enables no optional device features. Push constants stay under the 128 bytes Vulkan 1.0 guarantees. A few extras are used only when the device has them:

| Extra                  | Fallback                        |
| ---------------------- | ------------------------------- |
| 4x MSAA                | No multisampling                |
| Mailbox present mode   | FIFO (vsync)                    |
| sRGB swapchain format  | First format the surface offers |
| 32-bit or 24-bit depth | 16-bit depth                    |
| Opaque composite alpha | Whatever the surface supports   |
| Validation layer       | Skipped, with a note on stderr  |

Discrete GPUs are preferred, but integrated, virtual and software devices are accepted too.

## Project layout

```
crates/
  core/     maths, colours, frustum culling
  world/    blocks, terrain, chunk meshing
  render/   Vulkan device, swapchain, pipelines, sky, chunk and HUD drawing
  input/    camera and the InputSource trait
  app/      window, frame loop, desktop input
shaders/vulkan/   GLSL sources, compiled to SPIR-V at build time
```

`crates/core` and `crates/world` have no window or graphics dependency, so their logic is covered by unit tests that run without a GPU. `cargo test --workspace` builds and runs them.

## Platform notes

The `render` crate takes a `RawWindowHandle` and never depends on `winit`, and `app` is the only crate that knows about the windowing system. On Android, a Vulkan surface comes from an `ANativeWindow`, so an Android build adds a module in `app` that passes that handle through. Input already goes through the `InputSource` trait, so touch support means a new implementation rather than camera changes.

Vulkan is not available on every Android device. Devices without a Vulkan 1.0 driver cannot run this build.

## License

MIT. See [LICENSE](LICENSE). Copyright (c) 2024 Achmad Daniel Syahputra.
