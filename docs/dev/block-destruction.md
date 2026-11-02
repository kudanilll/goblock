# Block destruction, crosshair, and visual effects

This document describes how block targeting, destruction, the screen crosshair, and break effects work in GoBlock.

## Overview

Block destruction allows the player to target and break blocks in the world while playing. The feature consists of four main parts:

1. Raycasting from the camera to detect targeted blocks in the world.
2. Breaking state management that accounts for varying block hardness and input duration.
3. Instant chunk remeshing for modified chunks and their border neighbors.
4. Visual feedback, including a central 2D crosshair, 3D block crack overlays, and 3D debris particles.

## Component breakdown

### 1. Raycasting (`crates/world/src/raycast.rs`)

Voxel targeting uses the fast voxel traversal algorithm described by Amanatides and Woo. A ray begins at the player camera position and steps through voxel boundaries along the view direction up to 5.0 blocks.

For each step along the grid:

- The ray inspects the block at integer cell coordinate `(x, y, z)`.
- If the block is solid or breakable (not `BlockId::Air` and not liquid), the ray stops and reports a `RaycastHit`.
- `RaycastHit` provides the hit block position (`IVec3`), the hit face (`Face`), and the exact distance from the camera.

If the ray reaches the maximum distance without hitting any solid block, it returns `None`.

### 2. Block hardness and digging state (`crates/world/src/block.rs`, `crates/app/src/game.rs`)

Each block type defines a base hardness value:

- Foliage (Leaves, GrassTuft): 0.25 seconds.
- Loose blocks (Dirt, Sand, Gravel, Snow): 0.5 seconds.
- Wood and Planks (Log, Wood): 0.8 seconds.
- Stone and mineral blocks (Stone, Sandstone, Ice): 1.2 seconds.
- Bedrock is unbreakable.

When the player holds the left mouse button while looking at a target block:

- In walking mode, delta time accumulates into `break_progress`: `progress += dt / hardness`.
- In flying mode, blocks break instantly on click.
- If the player targets a different block or releases the mouse button before completion, the accumulated progress resets to zero.
- Once `progress >= 1.0`, the game triggers block destruction and spawns debris particles.

### 3. Chunk updates, plant detachment, and water physics (`crates/world/src/world.rs`)

When a block is destroyed:

1. `World::set_block(pos, BlockId::Air)` sets the block at `pos` to `BlockId::Air` in the relevant chunk.
2. The chunk is immediately remeshed.
3. If the block sits on a chunk boundary (`x == 0 || x == 15` or `z == 0 || z == 15`), adjacent neighbor chunks are also remeshed so that boundary face culling and ambient occlusion update without seams.
4. If a foliage block above (`pos + IVec3::Y`) requires a solid floor (such as `BlockId::GrassTuft`), it detaches and becomes `BlockId::Air` as well, emitting its own particle burst.
5. If the cleared block sits at or below `SEA_LEVEL` adjacent to water, or if water rests directly above it, water fills the cell and cascades downward into any empty air blocks below.
6. The remeshed chunks emit `WorldEvent::Meshed`, prompting the renderer to upload new GPU buffers via `Renderer::upload_chunk`.

### 4. 2D crosshair (`crates/render/src/vk/renderer.rs`, `crates/app/src/main.rs`)

The crosshair renders in `AppMode::Playing` at the center of the viewport:

- The crosshair consists of two clean rectangles drawn through the 2D HUD pipeline (`build_hud_vertices`).
- The horizontal bar spans 12 pixels wide by 2 pixels high.
- The vertical bar spans 2 pixels wide by 12 pixels high.
- Both bars use semi-transparent white `[255, 255, 255, 220]` without an outline or drop shadow.

### 5. 3D crack overlay and particle system (`crates/render/src/vk/`, `crates/app/src/`)

Visual feedback during and after block breaking uses 3D rendering with depth testing enabled:

- **Crack overlay**:
  While a block is being broken (`0.0 < progress < 1.0`), a cube outline and crack pattern render over the targeted block. The overlay cube is slightly enlarged to 1.002 blocks to prevent z-fighting with the underlying terrain mesh. The crack intensity scales from stage 0 through stage 9 matching break progress.

- **Debris particles**:
  Upon block destruction, the game spawns 16 small cube debris particles at the block center. Each particle receives a random initial velocity vector directed outward and upward, affected by gravity over its 0.6-second lifespan. Particles take their primary tint from the broken block's dominant color palette (`BlockId::particle_color`).

## Decision log

1. **In-world 3D overlay vs 2D HUD progress bar**:
   - Chosen: In-world 3D crack cube and 3D debris particles.
   - Alternatives considered: Screen-space circular radial meter around the crosshair.
   - Reason: In-world 3D effects preserve depth perception and integrate naturally with existing scene depth testing.

2. **Variable block hardness vs uniform break speed**:
   - Chosen: Hardness defined per block type.
   - Alternatives considered: Flat 0.5-second break timer for all blocks.
   - Reason: Differentiates light foliage from dense stone while maintaining snappy feedback.

3. **Synchronous border remeshing vs deferred queue**:
   - Chosen: Immediate local remesh for edited chunks.
   - Alternatives considered: Placing edit remeshes at the back of the background worker queue.
   - Reason: Prevents visible latency between click and visual block removal. Remeshing a single chunk on the CPU takes less than 0.5 ms.
