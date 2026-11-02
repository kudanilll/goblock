#version 450
#extension GL_GOOGLE_include_directive : require

#include "common.glsl"

// Set per pipeline: 0 opaque, 1 cutout (drops transparent texels).
layout(constant_id = 0) const int ALPHA_MODE = 0;

layout(set = 0, binding = 1) uniform sampler2DArray uBlocks;

layout(location = 0) in vec3 vUv;
layout(location = 1) in vec3 vTint;
layout(location = 2) in vec3 vView;
layout(location = 3) in vec3 vShadow;
layout(location = 4) in float vOcclusion;
layout(location = 5) flat in uint vFace;

layout(location = 0) out vec4 outColor;
// The depth again, as a colour the water shader can read.
layout(location = 1) out float outDepth;

void main() {
    vec4 texel = texture(uBlocks, vUv);
    if (ALPHA_MODE == 1 && texel.a < 0.5) {
        discard;
    }

    float distance = length(vView);
    vec3 viewDir = vView / max(distance, 1e-4);
    vec3 sun = frame.sunDirection.xyz;
    vec3 normal = FACE_NORMALS[vFace];
    if (ALPHA_MODE == 1 && !gl_FrontFacing) {
        normal = -normal;
    }

    // Faces turned away from the sun are in their own shadow anyway.
    float lit = dot(normal, sun) > 0.0 ? sunlight(vShadow) : 0.0;
    float direct = max(dot(normal, sun), 0.0) * lit;
    vec3 world = frame.cameraPosition.xyz + vView;
    if (world.y < WATER_LEVEL && normal.y > -0.2) {
        float depthFade = exp(-(WATER_LEVEL - world.y) * 0.35);
        float c = caustics(world.xz, frame.cameraPosition.w) * depthFade;
        direct += direct * c * 0.35;
    }
    // Sky light arrives from above and light bounced off the ground from
    // below. Ambient occlusion only dims this indirect part.
    vec3 ambient = mix(GROUND_BOUNCE, SKY_AMBIENT, normal.y * 0.5 + 0.5) * vOcclusion;
    vec3 color = texel.rgb * vTint * (SUN_COLOR * direct + ambient);

    vec4 fog = fogBetween(viewDir, distance);
    outColor = vec4(tonemap(mix(color, fog.rgb, fog.a)), 1.0);
    outDepth = gl_FragCoord.z;
}
