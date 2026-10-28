# GoBlock

A block themed casual game written in Rust, rendered with Vulkan through `ash`.

The world is endless. Terrain is generated in real-world units (each block is 15 m) from fractal noise for continents, mountain ridges and hills, and climate noise for temperature and rainfall picks one of 21 biomes per column, with trees and grass where the climate allows them. Chunks stream in around the player on worker threads and are dropped again behind it. The player walks with WASD and mouse look, and the debug HUD shows position, FPS, the current biome and how many chunks are on the GPU.

Trees have rounded, tall, conifer, slender and spreading crowns, selected by biome and seed. Leaves and grass sway in the wind. Grass has fixed roots, bends away as the player walks through it, and straightens as the player moves away. Each tuft is one of four pixel-art sprites (short, medium, tall, or with seed heads), picked and mirrored at random. Its colour and density vary across the world, and it does not block movement. Shadows follow the animated vegetation.

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

Every launch starts a new world from a random seed. The seed is shown in the HUD and printed when the game starts. Pass a seed to play a world again; numbers (negative ones too) are used as they are, and any other text always maps to the same world:

```sh
cargo run --release -- 42
cargo run --release -- "my world"
```

### Controls

| Input                | Action                     |
| -------------------- | -------------------------- |
| `W` `A` `S` `D`      | Walk                       |
| Mouse                | Look around                |
| `Shift`              | Sprint                     |
| `Space`              | Jump; while flying, fly up |
| `Space` twice, quick | Start or stop flying       |
| `Ctrl` or `C`        | While flying, fly down     |
| `F11`                | Fullscreen                 |
| `Esc`                | Quit                       |

The player is a 0.6 × 1.8 block body that walks under gravity and collides with blocks on every side. Flying stops on its own when you touch down.

## Textures

Block textures are PNG files in `assets/textures/blocks/`, one per name in `goblock_world::Texture` (`stone.png`, `grass_top.png`, `leaves.png` and so on). Any square size works; each one is scaled to 64×64 when the game starts. A missing file gets a placeholder colour and a note on stderr. The four grass tuft sprites are built in as 32×32 pixel art, and optional `grass_tuft.png`, `grass_tuft_short.png`, `grass_tuft_tall.png` and `grass_tuft_seeds.png` files replace them. A small PNG keeps hard pixel edges when it is scaled up, so a 32×32 or 64×64 drawing works best for these. `assets/textures/PROMPTS.md` has a prompt for every texture.

## Shaders

GLSL lives in `shaders/vulkan/` and is compiled by `crates/render/build.rs` into `${OUT_DIR}` with `glslc`, then embedded in the binary. The shaders target Vulkan 1.0, so the output is SPIR-V 1.0. Debug builds turn on the validation layer when it is installed and print its messages to stderr.

Lighting is done in linear HDR and tone mapped at the end: sunlight with soft shadows from a sun shadow map, sky and ground light dimmed by ambient occlusion, a scattering sky with a sun disc, and fog that takes the colour of the sky behind the terrain. Water is drawn after the rest of the scene and reads it back: the floor shows through with ripples bending it, fading with the depth of water in between, and the surface reflects the terrain with screen-space reflections, falling back to the sky where the reflected ray leaves the screen. It reflects more at grazing angles and catches the sun. Under water, the view fades into murky blue-green.

## Hardware support

The renderer uses Vulkan 1.0 core plus `VK_KHR_surface` and `VK_KHR_swapchain`, and enables no optional device features. Push constants stay under the 128 bytes Vulkan 1.0 guarantees. A few extras are used only when the device has them:

| Extra                  | Fallback                                                  |
| ---------------------- | --------------------------------------------------------- |
| 4x MSAA                | No multisampling                                          |
| Mailbox present mode   | FIFO (vsync)                                              |
| sRGB swapchain format  | First format the surface offers                           |
| 32-bit or 24-bit depth | 16-bit depth                                              |
| Opaque composite alpha | Whatever the surface supports                             |
| Validation layer       | Skipped, with a note on stderr                            |
| Sun shadows            | 2048 px on discrete, 1024 px on integrated GPUs, else off |
| Water reflections      | Sky only on virtual and software devices                  |

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

The biome rules are adapted from terrain-diffusion-mc, also MIT. The water's screen-space ray march is adapted from the Revelation shader pack under the Apache License 2.0. See [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).
