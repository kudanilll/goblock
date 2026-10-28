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
layout(location = 6) in float vWaterDepth;

layout(location = 0) out vec4 outColor;

// Colour that water scatters back toward the eye, before lighting.
const vec3 WATER_COLOR = vec3(0.04, 0.22, 0.30);
// How much of the light from the floor is lost per block travelled
// through water.
const float WATER_ABSORPTION = 0.55;

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

// Surface normal of moving water: the slope of several crossing sine waves
// at world position `p`. Far away the ripples flatten out, since they are
// smaller than a pixel there and would only shimmer.
vec3 waveNormal(vec2 p, float time, float distance) {
    vec2 slope = vec2(0.0);
    slope += vec2(0.8, 0.6) * cos(dot(p, vec2(0.8, 0.6)) * 0.9 + time * 1.3) * 0.05;
    slope += vec2(-0.5, 0.86) * cos(dot(p, vec2(-0.5, 0.86)) * 1.7 + time * 1.9) * 0.04;
    slope += vec2(0.3, -0.95) * cos(dot(p, vec2(0.3, -0.95)) * 3.1 + time * 2.7) * 0.03;
    slope += vec2(-0.9, -0.42) * cos(dot(p, vec2(-0.9, -0.42)) * 5.3 + time * 3.6) * 0.02;
    slope += vec2(0.62, -0.78) * cos(dot(p, vec2(0.62, -0.78)) * 8.9 + time * 4.8) * 0.012;
    slope *= mix(1.0, 0.25, smoothstep(20.0, 140.0, distance));
    return normalize(vec3(-slope.x, 1.0, -slope.y));
}

void main() {
    // Water is shaded entirely here, so only textured blocks sample.
    vec4 texel = ALPHA_MODE == 2 ? vec4(1.0) : texture(uBlocks, vUv);
    if (ALPHA_MODE == 1 && texel.a < 0.5) {
        discard;
    }

    float distance = length(vView);
    vec3 viewDir = vView / max(distance, 1e-4);
    vec3 sun = frame.sunDirection.xyz;
    vec3 faceNormal = FACE_NORMALS[vFace];

    bool waterSurface = ALPHA_MODE == 2 && vFace == 0u;
    vec3 normal = waterSurface
        ? waveNormal(frame.cameraPosition.xz + vView.xz, frame.cameraPosition.w, distance)
        : faceNormal;

    // Faces turned away from the sun are in their own shadow anyway.
    float lit = dot(faceNormal, sun) > 0.0 ? sunlight(vShadow) : 0.0;

    vec3 color;
    float alpha = 1.0;
    if (ALPHA_MODE == 2) {
        // Seen from below, the surface faces the other way.
        bool fromBelow = dot(viewDir, normal) > 0.0;
        vec3 n = fromBelow ? -normal : normal;
        float facing = max(dot(-viewDir, n), 0.0);

        // The flatter the view, the more the surface reflects (Schlick's
        // approximation of the Fresnel term, for water's index of 1.33).
        float fresnel = 0.02 + 0.98 * pow(1.0 - facing, 5.0);
        vec3 reflected = reflect(viewDir, n);
        vec3 reflection = fromBelow
            ? WATER_FOG
            : skyColor(reflected)
                + SUN_COLOR * 8.0 * pow(max(dot(reflected, sun), 0.0), 600.0) * lit;

        // Light from the floor fades with the distance it travels through
        // the water (Beer-Lambert), so shallow water is clear and deep water
        // shows only its own colour. From below, that water lies behind the
        // camera and the underwater fog covers it.
        float path = vWaterDepth / max(facing, 0.2);
        float transmit = fromBelow ? 1.0 : exp(-WATER_ABSORPTION * path);
        vec3 scatter = WATER_COLOR * vTint
            * (SKY_AMBIENT + SUN_COLOR * max(sun.y, 0.0) * 0.3 * lit);

        // What the water adds over the floor behind it, as straight alpha.
        vec3 added = (1.0 - fresnel) * (1.0 - transmit) * scatter + fresnel * reflection;
        alpha = 1.0 - (1.0 - fresnel) * transmit;
        color = added / max(alpha, 1e-3);
    } else {
        float direct = max(dot(normal, sun), 0.0) * lit;
        // Sky light arrives from above and light bounced off the ground from
        // below. Ambient occlusion only dims this indirect part.
        vec3 ambient = mix(GROUND_BOUNCE, SKY_AMBIENT, normal.y * 0.5 + 0.5) * vOcclusion;
        color = texel.rgb * vTint * (SUN_COLOR * direct + ambient);
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
