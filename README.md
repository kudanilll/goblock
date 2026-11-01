# GoBlock

A block themed casual game written in Rust, rendered with Vulkan through `ash`.

The world is endless. Terrain is generated in real-world units (each block is 15 m) from fractal noise for continents, mountain ridges and hills, and climate noise for temperature and rainfall picks one of 21 biomes per column, with trees and grass where the climate allows them. Chunks stream in around the player on worker threads and are dropped again behind it. The player walks with WASD and mouse look, and the debug HUD shows position, FPS, the current biome, walk or fly mode, how many chunks are on the GPU and the seed.

The game opens on a title menu over a slideshow of background images. From there you start a saved world or create a new one, and each world remembers where you left the player.

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
sudo apt install vulkan-tools libvulkan-dev glslc
```

The build script looks for `glslc` under `VULKAN_SDK`, then in the usual Windows install folders, then on `PATH`. Set `VULKAN_SDK` if your SDK lives somewhere unusual.

## Running

```sh
cargo run --release
```

The first build compiles the Vulkan stack and every shader to SPIR-V, so it takes a few minutes.

The game opens on the title menu. `Play` leads to the world list, where you can start, create or delete a world.

### Worlds and saves

A new world takes a name and an optional seed. A blank seed picks a random one. Numbers (negative ones too) are used as they are, and any other text always maps to the same world. The HUD and the world list both show the seed, and either form can be typed in again to get the same terrain. The player starts on the ground at a dry, fairly flat spot near the origin.

Each world is a folder in `saves/`, under the directory the game runs from. Its `world.json` holds the format version, name, seed, timestamps and the player's position, view direction and flying state. The terrain is rebuilt from the seed, so nothing else needs storing. The game writes this file when you start the world, close the window, quit from the settings menu, return to the title menu, lose window focus, and automatically every 60 seconds while playing. `Delete` in the world list turns into `Confirm` on the first click and removes the folder on the second click.

If `saves/` has no worlds at startup, the game creates one called Survival World. A seed passed on the command line goes to that world and is ignored once saves exist:

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
| `Esc`                | Open the settings menu     |

The player is a 0.6 × 1.8 block body that walks under gravity and collides with blocks on every side. Flying stops on its own when you touch down.

### Menus

Every menu works with the mouse. The keys are:

| Screen     | Keys                                                                                                                             |
| ---------- | -------------------------------------------------------------------------------------------------------------------------------- |
| Title      | `Enter` or `Space` world list, `S` settings, `Esc` quit                                                                          |
| World list | `↑` `↓` select, `Enter` play, `Esc` back                                                                                         |
| New world  | Type to fill in a field, `Tab` or `↑` `↓` switch field, `Enter` create, `Esc` cancel                                             |
| Settings   | `↑` `↓` pick a row, `←` `→` change it, `Enter` step it up or flip a toggle, `Tab` or `E` next tab, `Q` previous tab, `Esc` close |

The `Create New World` and `Delete` buttons in the world list have no key.

Pressing `Esc` in a world opens settings over a blurred view of it. The player stops and chunks stop loading until you close the menu. Its `Title Menu` and `Quit Game` buttons save the world before leaving.

### Settings

Settings has Video, Controls and Audio tabs. Mouse sensitivity and Invert Vertical Look take effect. The field of view, view distance, water reflection, sunlight shadow and volume rows change their stored values but have no effect on the game yet, the game has no sound so far, and the keyboard rows only list the controls. Settings go back to their defaults on every launch.

## Textures

Block textures are PNG files in `assets/textures/blocks/`, one per name in `goblock_world::Texture` (`stone.png`, `grass_top.png`, `leaves.png` and so on). The ones in the repository are 16×16 and 32×32 pixel art in muted, natural colours. Any square size works; each one is scaled to 64×64 when the game starts, and a small image keeps its hard pixel edges. The files look darker than the game shows them, because sunlight and the tone curve brighten a lit top face by about two times. A missing file gets a placeholder colour and a note on stderr. Grass tufts use four transparent sprites (`grass_tuft.png`, `grass_tuft_short.png`, `grass_tuft_tall.png` and `grass_tuft_seeds.png`), and a missing one leaves its tufts invisible.

## Menu art and fonts

Menu backgrounds are PNG files in `assets/backgrounds/`. The game loads all of them in name order and scales each one to 640×360. The title, world list and new world screens, and settings opened from the title, show them as a slideshow: each image holds for 8 seconds, and the last 1.5 seconds crossfade into the next. With no images, those screens show a plain dark colour.

Text uses two TrueType fonts from `assets/fonts/`, embedded in the binary at build time: `eas-vhs.ttf` for the HUD and body text, and `upheavtt.ttf` for titles and headings. The new world screen only accepts printable ASCII.

## Shaders

GLSL lives in `shaders/vulkan/` and is compiled by `crates/render/build.rs` into `${OUT_DIR}` with `glslc`, then embedded in the binary. The shaders target Vulkan 1.0, so the output is SPIR-V 1.0. Debug builds turn on the validation layer when it is installed and print its messages to stderr.

Lighting is done in linear HDR and tone mapped at the end: sunlight with soft shadows from a sun shadow map, sky and ground light dimmed by ambient occlusion, a scattering sky with a sun disc, and fog that takes the colour of the sky behind the terrain. Water is drawn after the rest of the scene and reads it back: the floor shows through with ripples bending it, fading with the depth of water in between, and the surface reflects the terrain with screen-space reflections, falling back to the sky where the reflected ray leaves the screen. It reflects more at grazing angles and catches the sun. Moving caustics light anything under water. With the camera under water, the view fades into murky blue-green, the picture ripples and shafts of sunlight fall through it. Settings opened from a world blurs and darkens the game behind the menu.

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
  render/   Vulkan device, swapchain, pipelines, chunk buffers, sky and HUD drawing,
            font atlas
  input/    camera, player movement and collision, and the InputSource trait
  app/      window, frame loop, desktop input, asset loading, menus, settings,
            world saves
shaders/vulkan/     GLSL sources, compiled to SPIR-V at build time
assets/textures/    block textures
assets/backgrounds/ menu slideshow images
assets/fonts/       TrueType fonts, embedded at build time
saves/              world saves, created by the game and ignored by git
```

`crates/core`, `crates/world` and `crates/input` have no window or graphics dependency, so their logic is covered by unit tests that run without a GPU. `cargo test --workspace` builds and runs them.

## Platform notes

The `render` crate takes a `RawWindowHandle` and never depends on `winit`, and `app` is the only crate that knows about the windowing system. On Android, a Vulkan surface comes from an `ANativeWindow`, so an Android build adds a module in `app` that passes that handle through. Input in a world already goes through the `InputSource` trait, so touch support there means a new implementation rather than camera changes. The menus read `winit` key codes and mouse positions directly, and saves use a path relative to the working directory, so both need Android versions.

Vulkan is not available on every Android device. Devices without a Vulkan 1.0 driver cannot run this build.

## License

MIT. See [LICENSE](LICENSE). Copyright (c) 2024 Achmad Daniel Syahputra.

The biome rules are adapted from terrain-diffusion-mc, also MIT. The water's screen-space ray march is adapted from the Revelation shader pack under the Apache License 2.0. The EAS VHS font is under CC BY 3.0, and the Upheaval font stays under its author's freeware terms. See [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).
