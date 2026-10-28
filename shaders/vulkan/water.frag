#version 450
#extension GL_GOOGLE_include_directive : require

// Water, drawn over the finished scene. It reads that scene to see the floor
// through the surface and to find what the surface reflects.
//
// The screen-space ray march in traceReflection is adapted, with changes,
// from the Revelation shader pack (Apache License 2.0). See
// THIRD-PARTY-NOTICES.md.

#include "common.glsl"

// The scene behind the water: tone-mapped colour, and depth as
// gl_FragCoord.z with 1 where only sky shows.
layout(set = 0, binding = 3) uniform sampler2D uSceneColor;
layout(set = 0, binding = 4) uniform sampler2D uSceneDepth;

layout(location = 1) in vec3 vTint;
layout(location = 2) in vec3 vView;
layout(location = 3) in vec3 vShadow;
layout(location = 5) flat in uint vFace;

layout(location = 0) out vec4 outColor;

// Colour that water scatters back toward the eye, before lighting.
const vec3 WATER_COLOR = vec3(0.04, 0.22, 0.30);
// How much of the light from the floor is lost per block travelled
// through water.
const float WATER_ABSORPTION = 0.55;
// A reflected ray is sampled this many times across the screen, then the
// hit is narrowed down by halving the last step this many times.
const int RAY_STEPS = 16;
const int RAY_REFINE = 4;
// Longest reflected ray, in blocks, and how close to the camera (in clip
// w, past the 0.1 near plane) a ray coming back toward it may get.
const float RAY_LENGTH = 512.0;
const float RAY_NEAR = 0.2;

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

// Screen position of a clip-space point: uv in xy, depth in z. Both change
// linearly along a straight line on screen, so a ray can be stepped there.
vec3 toScreen(vec4 clip) {
    return vec3(clip.xy / clip.w * 0.5 + 0.5, clip.z / clip.w);
}

float sceneDepth(vec2 uv) {
    return textureLod(uSceneDepth, uv, 0.0).r;
}

// Follows a ray from camera-relative `origin` in direction `dir` across the
// screen until it passes behind the scene. Returns the uv of the hit in xy
// and how far to trust it in z, which is 0 for a miss.
vec3 traceReflection(vec3 origin, vec3 dir, float jitter) {
    vec4 start = frame.viewProjection * vec4(origin, 1.0);
    vec4 toward = frame.viewProjection * vec4(dir, 0.0);
    // Clip w is the distance in front of the camera. A ray turning back
    // toward the camera has to end before it crosses the near plane.
    float len = RAY_LENGTH;
    if (toward.w < 0.0) {
        len = clamp((RAY_NEAR - start.w) / toward.w, 0.0, len);
    }
    vec3 a = toScreen(start);
    vec3 delta = toScreen(start + toward * len) - a;

    // Cut the ray where it leaves the screen.
    vec2 safe = mix(vec2(1e-6), delta.xy, greaterThan(abs(delta.xy), vec2(1e-6)));
    vec2 room = (step(0.0, safe) - a.xy) / safe;
    delta *= clamp(min(room.x, room.y), 0.0, 1.0);

    // Each pixel starts its steps at a different offset, which turns the
    // banding of coarse steps into fine noise.
    float before = 0.0;
    for (int i = 1; i <= RAY_STEPS; i++) {
        float t = (float(i) - jitter) / float(RAY_STEPS);
        vec3 p = a + delta * t;
        float scene = sceneDepth(p.xy);
        if (p.z > scene) {
            // The surface has to lie between this step and the last one.
            // Otherwise the ray only passed behind something in front.
            if (scene < a.z + delta.z * before) {
                before = t;
                continue;
            }
            float lo = before;
            float hi = t;
            for (int j = 0; j < RAY_REFINE; j++) {
                float mid = 0.5 * (lo + hi);
                vec3 m = a + delta * mid;
                if (m.z > sceneDepth(m.xy)) {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            vec2 uv = a.xy + delta.xy * hi;
            // Fade out near the edges, where the reflected scene runs out.
            vec2 edge = min(uv, 1.0 - uv);
            return vec3(uv, smoothstep(0.0, 0.08, min(edge.x, edge.y)));
        }
        before = t;
    }
    return vec3(0.0);
}

void main() {
    float distance = length(vView);
    vec3 viewDir = vView / max(distance, 1e-4);
    vec3 sun = frame.sunDirection.xyz;
    vec3 faceNormal = FACE_NORMALS[vFace];
    vec3 normal = vFace == 0u
        ? waveNormal(frame.cameraPosition.xz + vView.xz, frame.cameraPosition.w, distance)
        : faceNormal;

    // Faces turned away from the sun are in their own shadow anyway.
    float lit = dot(faceNormal, sun) > 0.0 ? sunlight(vShadow) : 0.0;

    // Seen from below, the surface faces the other way.
    bool fromBelow = dot(viewDir, normal) > 0.0;
    vec3 n = fromBelow ? -normal : normal;
    float facing = max(dot(-viewDir, n), 0.0);
    // The flatter the view, the more the surface reflects (Schlick's
    // approximation of the Fresnel term, for water's index of 1.33).
    float fresnel = 0.02 + 0.98 * pow(1.0 - facing, 5.0);

    // The ripples bend the view of what lies below. A bent lookup that lands
    // on something in front of the water falls back to the straight one.
    vec2 screen = gl_FragCoord.xy / vec2(textureSize(uSceneColor, 0));
    vec2 bent = screen + (normal.xz - faceNormal.xz) * 0.04;
    float floorDepth = sceneDepth(bent);
    if (floorDepth < gl_FragCoord.z) {
        bent = screen;
        floorDepth = sceneDepth(screen);
    }
    vec3 behind = textureLod(uSceneColor, bent, 0.0).rgb;

    // Light from the floor fades with the distance it travels through the
    // water (Beer-Lambert), so shallow water is clear and deep water shows
    // only its own colour. From below, that water lies behind the camera
    // and the underwater fog covers it.
    vec4 floorPoint = frame.inverseViewProjection * vec4(bent * 2.0 - 1.0, floorDepth, 1.0);
    float thickness = max(length(floorPoint.xyz / floorPoint.w) - distance, 0.0);
    float transmit = fromBelow ? 1.0 : exp(-WATER_ABSORPTION * thickness);
    vec3 scatter = WATER_COLOR * vTint
        * (SKY_AMBIENT + SUN_COLOR * max(sun.y, 0.0) * 0.3 * lit);

    vec3 reflected = reflect(viewDir, n);
    vec3 reflection = tonemap(fromBelow
        ? WATER_FOG
        : skyColor(reflected) + SUN_COLOR * 8.0 * pow(max(dot(reflected, sun), 0.0), 600.0) * lit);
    if (!fromBelow && frame.fog.w > 0.5) {
        // Rays are kept above the surface, which would hide anything below.
        vec3 ray = normalize(vec3(reflected.x, max(reflected.y, 0.02), reflected.z));
        // Interleaved gradient noise (Jorge Jimenez) for the step offset.
        float jitter = fract(52.9829189 * fract(dot(gl_FragCoord.xy, vec2(0.06711056, 0.00583715))));
        vec3 hit = traceReflection(vView, ray, jitter);
        reflection = mix(reflection, textureLod(uSceneColor, hit.xy, 0.0).rgb, hit.z);
    }

    // The scene is already tone mapped, so everything is mixed in display
    // range, as blending over it would.
    vec3 color = mix(behind, tonemap(scatter), 1.0 - transmit);
    color = mix(color, reflection, fresnel);

    vec4 fog = fogBetween(viewDir, distance);
    outColor = vec4(mix(color, tonemap(fog.rgb), fog.a), 1.0);
}
