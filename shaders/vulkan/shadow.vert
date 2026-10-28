#version 450
#extension GL_GOOGLE_include_directive : require

#include "common.glsl"

layout(location = 0) in uvec4 aPosition;
layout(location = 1) in uvec4 aUvLayerShade;

layout(push_constant) uniform Push {
    // xyz: the chunk's origin relative to the camera.
    vec4 chunkOffset;
} pc;

layout(location = 0) out vec3 vUv;

void main() {
    vec3 view = pc.chunkOffset.xyz + vec3(aPosition.xyz) / 16.0;
    gl_Position = frame.shadowMatrix * vec4(view, 1.0);
    vUv = vec3(aUvLayerShade.xyz);
}
