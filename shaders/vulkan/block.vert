#version 450
#extension GL_GOOGLE_include_directive : require

#include "common.glsl"

// xyz: position inside the chunk in sixteenths of a block. w: the face, an
// index into FACE_NORMALS.
layout(location = 0) in uvec4 aPosition;
// Texture u and v, texture array layer, and ambient occlusion out of 255.
layout(location = 1) in uvec4 aUvLayerShade;
layout(location = 2) in vec4 aTint;

layout(push_constant) uniform Push {
    // xyz: the chunk's origin relative to the camera.
    vec4 chunkOffset;
} pc;

layout(location = 0) out vec3 vUv;
layout(location = 1) out vec3 vTint;
layout(location = 2) out vec3 vView;
layout(location = 3) out vec3 vShadow;
layout(location = 4) out float vOcclusion;
layout(location = 5) flat out uint vFace;
// Blocks of water below a water face; unused for other blocks.
layout(location = 6) out float vWaterDepth;

void main() {
    vec3 view = pc.chunkOffset.xyz + vec3(aPosition.xyz) / 16.0;
    gl_Position = frame.viewProjection * vec4(view, 1.0);

    // Look the shadow up slightly off the surface, so a face does not
    // shadow itself.
    vec3 lookup = view + FACE_NORMALS[aPosition.w] * 0.08;
    vec4 shadow = frame.shadowMatrix * vec4(lookup, 1.0);
    vShadow = vec3(shadow.xy * 0.5 + 0.5, shadow.z);

    vUv = vec3(aUvLayerShade.xyz);
    vTint = aTint.rgb;
    vOcclusion = float(aUvLayerShade.w) / 255.0;
    vView = view;
    vFace = aPosition.w;
    vWaterDepth = aTint.a * 255.0;
}
