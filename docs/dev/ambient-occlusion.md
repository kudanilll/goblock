# Ambient occlusion

This document describes how baked voxel ambient occlusion and inline screen-space ambient occlusion (SSAO) work in GoBlock.

## Overview

GoBlock uses a hybrid ambient occlusion model. Voxel corners and ground contact points are shaded on the CPU during chunk meshing, and contact shadows around dynamic camera angles and fine crevices are evaluated on the GPU during the composite pass.

The system has four main parts:

1. Corner occlusion sampling during chunk meshing in `crates/world/src/mesh.rs`.
2. Ground contact shading for vegetation tufts in `crates/world/src/mesh.rs`.
3. Frame uniform state delivery through `uiState.y` in `crates/render/src/vk/renderer.rs` and `shaders/common.glsl`.
4. Inline 8-sample SSAO evaluation in `shaders/scene.glsl` and `shaders/fragment/composite.glsl`.

## Baked voxel ambient occlusion

Chunk geometry bakes corner occlusion directly into vertex data so static block corners cost nothing during fragment shading.

### Corner occlusion calculation

When meshing a visible quad in `Quad::emit`, the mesher checks the four corners of the face using `Quad::corner_occlusion`. For each corner, it inspects three neighboring blocks in the plane directly in front of the face:

- Two side neighbors touching the corner edges.
- One diagonal corner neighbor.

If both side neighbors are opaque blocks, light cannot reach the corner from either side, so the unblocked count is 0. Otherwise, the unblocked count is 3 minus the number of opaque blocks among the two sides and the diagonal.

The unblocked count (0 to 3) indexes into `AO_LEVELS`:

```rust
const AO_LEVELS: [u8; 4] = [115, 166, 209, 255];
```

Level 0 (three occluding blocks) provides the darkest crevice shade (115), while level 3 (zero occluding blocks) represents an open face (255).

### Diagonal flip for interpolation

OpenGL and Vulkan interpolate vertex attributes across two triangles per quad. If a quad has two opposite dark corners and two bright corners, splitting the quad along one diagonal produces an asymmetric crease artifact across the face.

The mesher resolves this by comparing diagonal sums:

```rust
let order = if ao[0] + ao[2] < ao[1] + ao[3] {
    [1, 2, 3, 0]
} else {
    [0, 1, 2, 3]
};
```

The quad splits along the brighter diagonal, making the lighting gradient smooth across the surface.

### Vegetation ground contact

Crossed grass tufts (`BlockId::GrassTuft`) consist of two intersecting vertical quads generated in `emit_grass`. Unlike solid blocks, grass cards do not have standard corner neighbors.

To avoid tufts looking detached from the ground, vertices sitting at `y == 0` receive a contact shade of `AO_LEVELS[1]` (166). The top vertices at `y == 16` receive 255. This darkens the base of the plant where it meets the soil while keeping the upper leaves bright.

The shade value packs into byte 3 of `ChunkVertex.uv_layer_shade`, keeping each chunk vertex at 16 bytes. In `shaders/fragment/block.glsl`, this value modulates incoming sky ambient and ground bounce light:

```glsl
vec3 ambient = mix(GROUND_BOUNCE, SKY_AMBIENT, normal.y * 0.5 + 0.5) * vOcclusion;
```

## Screen-space ambient occlusion

While baked voxel AO covers static 90-degree block intersections, screen-space ambient occlusion adds dynamic contact shadows based on scene depth. It runs on the composite pass when drawing the scene to the swapchain.

### Pipeline architecture

GoBlock targets Vulkan 1.0 core and respects the 4096 device memory allocation budget. Creating dedicated offscreen AO render targets and bilateral blur passes would require extra framebuffers, image views, and render passes.

Instead, SSAO executes inline within `shaders/fragment/composite.glsl`. The composite pass already samples `uSceneColor` (binding 3) and `uSceneDepth` (binding 4) to handle underwater distortion and background blurring. SSAO evaluates depth samples directly from `uSceneDepth` and modulates the sampled scene color in place.

### Normal reconstruction

The scene pass does not write a separate normal buffer. The SSAO shader reconstructs view-space geometric surface normals from camera-relative reconstructed positions using screen-space partial derivatives:

```glsl
vec3 point = scenePosition(uv, depth);
vec3 dPdx = dFdx(point);
vec3 dPdy = dFdy(point);
vec3 normal = cross(dPdx, dPdy);
normal *= inversesqrt(dot(normal, normal));
if (dot(normal, point) > 0.0) {
    normal = -normal;
}
```

This derives the exact polygon normal for voxel faces without adding memory bandwidth overhead during the scene pass.

### Sampling kernel and rotation noise

The sampling kernel uses 8 constant sample points distributed on a hemisphere oriented along the surface normal:

```glsl
const vec3 SSAO_SAMPLES[8] = vec3[](
    vec3( 0.35,  0.15, 0.30),
    vec3(-0.25,  0.35, 0.45),
    vec3( 0.20, -0.40, 0.50),
    vec3(-0.35, -0.20, 0.60),
    vec3( 0.50,  0.45, 0.40),
    vec3(-0.60, -0.30, 0.50),
    vec3( 0.30, -0.60, 0.55),
    vec3(-0.45,  0.55, 0.65)
);
```

To eliminate banding patterns without an external noise texture, the shader rotates the tangent basis for each screen pixel using Interleaved Gradient Noise:

```glsl
float interleavedGradientNoise(vec2 screenPos) {
    return fract(52.9829189 * fract(dot(screenPos, vec2(0.06711056, 0.00583715))));
}
```

Each sample point is projected into clip space via `frame.viewProjection`. The shader samples `uSceneDepth` at the projected UV coordinate and converts the buffer depth to linear view depth with `viewDepth`.

### Range checking and edge cases

If a background surface sits behind a foreground silhouette, testing depth differences naively would produce dark halos around foreground edges.

The shader applies a range check based on a 0.65-block radius:

```glsl
float distDiff = sampleLinear - actualLinear;
if (distDiff > 0.02 && distDiff < radius) {
    float rangeCheck = smoothstep(radius, 0.0, distDiff);
    occlusion += rangeCheck;
}
```

Edge cases are handled as follows:

- **Sky**: Pixels with depth equal to or greater than 0.9999 bypass occlusion testing and return 1.0.
- **Screen bounds**: Sample points whose projected coordinates fall outside `0.0..1.0` are skipped.
- **Fog distance fade**: As view distance increases, occlusion fades out between 25% and 70% of `frame.fog.y`. This avoids noise on distant hills and blends geometry into aerial perspective.
- **Darkening ceiling**: Total occlusion scales by 0.45, preserving an ambient lighting floor so crevices do not turn pure black.

## Uniforms and settings control

SSAO state passes from the application layer to the shaders through the per-frame uniform buffer:

1. `Settings` in `crates/app/src/settings.rs` defines `pub ambient_occlusion: bool` (default: `true`).
2. The Video tab in the settings menu provides an "Ambient Occlusion" toggle row.
3. `ActiveState::frame` in `crates/app/src/main.rs` copies this flag to `FrameView.ambient_occlusion`.
4. `frame_uniforms` in `crates/render/src/vk/renderer.rs` packs this boolean into `uiState.y` (byte offset 260).
5. The `Frame` uniform block maintains its 272-byte std140 layout.

In `shaders/fragment/composite.glsl`, if `frame.uiState.y <= 0.5`, SSAO evaluation is skipped entirely, matching the original raw scene color presentation.
