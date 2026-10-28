# GoBlock

A block themed casual game written in Rust, rendered with Vulkan through `ash`.

The world is endless. Terrain is generated in real-world units (each block is 15 m) from fractal noise for continents, mountain ridges and hills, and climate noise for temperature and rainfall picks one of 21 biomes per column, with trees where the climate allows them. Chunks stream in around the camera on worker threads and are dropped again behind it. The camera flies with WASD and mouse look, and the debug HUD shows position, FPS, the current biome and how many chunks are on the GPU.

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

The world seed defaults to 1. Pass another number to get a different world:

```sh
cargo run --release -- 42
```

### Controls

| Input           | Action      |
| --------------- | ----------- |
| `W` `A` `S` `D` | Move        |
| Mouse           | Look around |
| `Shift`         | Sprint      |
| `Space`         | Fly up      |
| `Ctrl` or `C`   | Fly down    |
| `Esc`           | Quit        |

## Textures

Block textures are PNG files in `assets/textures/blocks/`, one per name in `goblock_world::Texture` (`stone.png`, `grass_top.png`, `leaves.png` and so on). Any square size works; each one is scaled to 64×64 when the game starts. A missing file gets a placeholder colour and a note on stderr. `assets/textures/PROMPTS.md` has a prompt for every texture.

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

```text
crates/
  core/     frustum culling
  world/    blocks, terrain and biomes, chunk generation, meshing and streaming
  render/   Vulkan device, swapchain, pipelines, chunk buffers, sky and HUD drawing
  input/    camera and the InputSource trait
  app/      window, frame loop, desktop input, texture loading
shaders/vulkan/   GLSL sources, compiled to SPIR-V at build time
assets/textures/  block textures and the prompts used to make them
```

`crates/core`, `crates/world` and `crates/input` have no window or graphics dependency, so their logic is covered by unit tests that run without a GPU. `cargo test --workspace` builds and runs them.

## Platform notes

The `render` crate takes a `RawWindowHandle` and never depends on `winit`, and `app` is the only crate that knows about the windowing system. On Android, a Vulkan surface comes from an `ANativeWindow`, so an Android build adds a module in `app` that passes that handle through. Input already goes through the `InputSource` trait, so touch support means a new implementation rather than camera changes.

Vulkan is not available on every Android device. Devices without a Vulkan 1.0 driver cannot run this build.

## License

MIT. See [LICENSE](LICENSE). Copyright (c) 2024 Achmad Daniel Syahputra.

The biome rules are adapted from terrain-diffusion-mc, also MIT; see [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).
