#version 450
#extension GL_GOOGLE_include_directive : require

#include "common.glsl"

// Set per pipeline: 0 opaque, 1 cutout (drops transparent texels),
// 2 translucent (water, alpha blended).
layout(constant_id = 0) const int ALPHA_MODE = 0;

layout(set = 0, binding = 1) uniform sampler2DArray uBlocks;
layout(set = 0, binding = 2) uniform sampler2DShadow uShadow;

layout(location = 0) in vec3 vUv;
layout(location = 1) in vec3 vTint;
layout(location = 2) in vec3 vView;
layout(location = 3) in vec3 vShadow;
layout(location = 4) in float vOcclusion;
layout(location = 5) flat in uint vFace;

layout(location = 0) out vec4 outColor;

// Fraction of sunlight reaching the shadow map position `coord`, from a
// 3x3 block of depth comparisons that softens the shadow's edge.
float sunlight(vec3 coord) {
    float texel = frame.sunDirection.w;
    if (texel == 0.0) {
        return 1.0;
    }
    float lit = 0.0;
    for (int y = -1; y <= 1; y++) {
        for (int x = -1; x <= 1; x++) {
            lit += texture(uShadow, vec3(coord.xy + vec2(x, y) * texel, min(coord.z, 1.0)));
        }
    }
    return lit / 9.0;
}

// Surface normal of gently moving water: the slope of a few crossing sine
// waves at world position `p`.
vec3 waveNormal(vec2 p, float time) {
    vec2 slope = vec2(0.8, 0.6) * cos(dot(p, vec2(0.8, 0.6)) * 1.3 + time * 1.7) * 0.06;
    slope += vec2(-0.5, 0.86) * cos(dot(p, vec2(-0.5, 0.86)) * 2.1 + time * 2.3) * 0.04;
    slope += vec2(0.3, -0.95) * cos(dot(p, vec2(0.3, -0.95)) * 3.7 + time * 3.1) * 0.025;
    return normalize(vec3(-slope.x, 1.0, -slope.y));
}

void main() {
    vec4 texel = texture(uBlocks, vUv);
    if (ALPHA_MODE == 1 && texel.a < 0.5) {
        discard;
    }

    float distance = length(vView);
    vec3 viewDir = vView / max(distance, 1e-4);
    vec3 sun = frame.sunDirection.xyz;
    vec3 faceNormal = FACE_NORMALS[vFace];

    bool waterSurface = ALPHA_MODE == 2 && vFace == 0u;
    vec3 normal = waterSurface
        ? waveNormal(frame.cameraPosition.xz + vView.xz, frame.cameraPosition.w)
        : faceNormal;

    // Faces turned away from the sun are in their own shadow anyway.
    float lit = dot(faceNormal, sun) > 0.0 ? sunlight(vShadow) : 0.0;
    float direct = max(dot(normal, sun), 0.0) * lit;
    // Sky light arrives from above and light bounced off the ground from
    // below. Ambient occlusion only dims this indirect part.
    vec3 ambient = mix(GROUND_BOUNCE, SKY_AMBIENT, normal.y * 0.5 + 0.5) * vOcclusion;
    vec3 color = texel.rgb * vTint * (SUN_COLOR * direct + ambient);
    float alpha = ALPHA_MODE == 2 ? texel.a : 1.0;

    if (waterSurface) {
        // The flatter the view, the more sky the water reflects (Schlick's
        // approximation of the Fresnel term).
        vec3 reflected = reflect(viewDir, normal);
        float fresnel = 0.02 + 0.98 * pow(1.0 - max(dot(-viewDir, normal), 0.0), 5.0);
        vec3 reflection = skyColor(reflected);
        reflection += SUN_COLOR * 6.0 * pow(max(dot(reflected, sun), 0.0), 400.0) * lit;
        color = mix(color, reflection, fresnel);
        alpha = mix(alpha, 1.0, fresnel);
    }

    if (frame.fog.z > 0.5) {
        // Under water, light dies out within a few blocks.
        color = mix(color, WATER_FOG, 1.0 - exp(-distance * 0.12));
    } else {
        // Aerial perspective: distant terrain takes on the colour of the sky
        // behind it. A light haze starts right away, and the fog closes
        // completely before the edge of the loaded world.
        float haze = 1.0 - exp(-distance * 0.002);
        float fog = max(smoothstep(frame.fog.x, frame.fog.y, distance), haze * 0.5);
        color = mix(color, skyColor(viewDir), fog);
    }

    outColor = vec4(tonemap(color), alpha);
}
