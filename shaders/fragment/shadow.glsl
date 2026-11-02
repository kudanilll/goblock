#version 450

// Set per pipeline: 0 opaque, 1 cutout. Cutout drops transparent texels, so
// sunlight passes through the gaps between leaves.
layout(constant_id = 0) const int ALPHA_MODE = 0;

layout(set = 0, binding = 1) uniform sampler2DArray uBlocks;

layout(location = 0) in vec3 vUv;

void main() {
    if (ALPHA_MODE == 1 && texture(uBlocks, vUv).a < 0.5) {
        discard;
    }
}
