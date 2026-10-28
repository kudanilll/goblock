#version 450

// Position inside the chunk in sixteenths of a block; w is unused.
layout(location = 0) in uvec4 aPosition;
// Texture u and v, texture array layer, and brightness out of 255.
layout(location = 1) in uvec4 aUvLayerShade;
layout(location = 2) in vec4 aTint;

// Shared with block.frag. Keep the block within 128 bytes, the most Vulkan
// 1.0 guarantees for push constants.
layout(push_constant) uniform Push {
    mat4 viewProjection;
    // xyz: the chunk's origin relative to the camera.
    vec4 chunkOffset;
    vec4 fogColor;
    // x: distance where fog starts, y: distance where it hides everything.
    vec4 fog;
} pc;

layout(location = 0) out vec3 vUv;
layout(location = 1) out vec3 vColor;
layout(location = 2) out vec3 vView;

void main() {
    vec3 view = pc.chunkOffset.xyz + vec3(aPosition.xyz) / 16.0;
    gl_Position = pc.viewProjection * vec4(view, 1.0);
    vUv = vec3(aUvLayerShade.xyz);
    vColor = aTint.rgb * (float(aUvLayerShade.w) / 255.0);
    vView = view;
}
