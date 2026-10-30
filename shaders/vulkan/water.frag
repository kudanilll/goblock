#version 450
#extension GL_GOOGLE_include_directive : require

// Water, drawn over the finished scene. It reads that scene to see the floor
// through the surface and to find what the surface reflects.
//
// The screen-space ray march in traceReflection is adapted, with changes,
// from the Revelation shader pack (Apache License 2.0). See
// THIRD-PARTY-NOTICES.md.

#include "common.glsl"
#include "scene.glsl"

layout(location = 1) in vec3 vTint;
layout(location = 2) in vec3 vView;
layout(location = 3) in vec3 vShadow;
layout(location = 5) flat in uint vFace;

layout(location = 0) out vec4 outColor;

// Natural, realistic water body scattering color.
const vec3 WATER_COLOR = vec3(0.03, 0.08, 0.13);
// How much light is absorbed per block travelled through water.
const float WATER_ABSORPTION = 0.35;
// A reflected ray is sampled this many times across the screen, then the
// hit is narrowed down by halving the last step this many times.
const int RAY_STEPS = 16;
const int RAY_REFINE = 5;
// Longest reflected ray, in blocks, and how close to the camera (in clip
// w, past the 0.1 near plane) a ray coming back toward it may get.
const float RAY_LENGTH = 512.0;
const float RAY_NEAR = 0.2;

// Fluid, natural wave normals without repetitive planar banding.
// Far away the ripples flatten smoothly so the horizon stays calm without shimmering.
vec3 waveNormal(vec2 p, float time, float distance) {
    vec2 slope = vec2(0.0);
    // Large wind swell with gentle domain curve to prevent repeating grid lines
    vec2 pWarp = p + vec2(sin(p.y * 0.15 + time * 0.6), cos(p.x * 0.15 + time * 0.5)) * 0.8;

    vec2 d1 = vec2(0.78, 0.62);
    float ph1 = dot(pWarp, d1) * 0.75 + time * 1.3;
    slope += d1 * cos(ph1) * 0.045;

    vec2 d2 = vec2(-0.65, 0.76);
    float ph2 = dot(p, d2) * 1.35 + time * 1.7;
    slope += d2 * cos(ph2) * 0.035;

    vec2 d3 = vec2(0.38, -0.92);
    float ph3 = dot(pWarp, d3) * 2.6 + time * 2.4;
    slope += d3 * cos(ph3) * 0.025;

    vec2 d4 = vec2(-0.85, -0.53);
    float ph4 = dot(p, d4) * 4.8 + time * 3.2;
    slope += d4 * cos(ph4) * 0.016;

    vec2 d5 = vec2(0.58, -0.81);
    float ph5 = dot(pWarp, d5) * 8.2 + time * 4.2;
    slope += d5 * cos(ph5) * 0.008;

    // Smoothly flatten far waves to zero to prevent horizon shimmering and striped noise
    slope *= 1.0 - smoothstep(12.0, 80.0, distance);
    return normalize(vec3(-slope.x, 1.0, -slope.y));
}

// Screen position of a clip-space point: uv in xy, depth in z. Both change
// linearly along a straight line on screen, so a ray can be stepped there.
vec3 toScreen(vec4 clip) {
    return vec3(clip.xy / clip.w * 0.5 + 0.5, clip.z / clip.w);
}

// Follows a ray from camera-relative `origin` in direction `dir` across the
// screen until it passes behind the scene. Returns the uv of the hit in xy
// and how far to trust it in z, which is 0 for a miss.
vec3 traceReflection(vec3 origin, vec3 dir, float side) {
    vec4 start = frame.viewProjection * vec4(origin, 1.0);
    vec4 toward = frame.viewProjection * vec4(dir, 0.0);
    if (start.w <= RAY_NEAR) {
        return vec3(0.0);
    }
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

    float before = 0.0;
    float previousGap = viewDepth(a.z) - viewDepth(sceneDepth(a.xy));
    for (int i = 1; i <= RAY_STEPS; i++) {
        float t = float(i) / float(RAY_STEPS);
        vec3 p = a + delta * t;
        float scene = sceneDepth(p.xy);
        float gap = viewDepth(p.z) - viewDepth(scene);
        if (scene < 1.0 && gap > 0.0 && previousGap <= 0.0) {
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
            float depth = sceneDepth(uv);
            vec3 hitPoint = scenePosition(uv, depth);
            vec3 rayPoint = scenePosition(uv, a.z + delta.z * hi);
            // A crossing at a silhouette is not necessarily a ray hit.
            float tolerance = 0.15 + 0.002 * length(hitPoint);
            float confidence = 1.0 - smoothstep(0.0, tolerance, length(hitPoint - rayPoint));
            if (depth >= 1.0 || side * (hitPoint.y - origin.y) < 0.0
                || dot(hitPoint - origin, dir) < 0.2) {
                return vec3(0.0);
            }
            confidence *= sceneContinuity(uv);
            vec2 edge = min(uv, 1.0 - uv) * vec2(textureSize(uSceneDepth, 0));
            confidence *= smoothstep(1.0, 8.0, min(edge.x, edge.y));
            return vec3(sceneUv(uv), confidence);
        }
        before = t;
        previousGap = gap;
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

    // The mesh determines which medium the ray leaves, even at grazing angles.
    bool fromBelow = dot(viewDir, faceNormal) > 0.0;
    float grazing = smoothstep(0.0, 0.15, abs(dot(viewDir, faceNormal)));
    normal = normalize(mix(faceNormal, normal, grazing));
    vec3 n = fromBelow ? -normal : normal;
    float facing = clamp(dot(-viewDir, n), 0.0, 1.0);

    // Fresnel reflectance. From below, total internal reflection occurs outside
    // Snell's window (critical angle ~48.6 degrees).
    float eta = fromBelow ? 1.333 : 1.0 / 1.333;
    float sin2 = eta * eta * (1.0 - facing * facing);
    float fresnel = 1.0;
    if (sin2 < 1.0) {
        float transmittedCos = sqrt(1.0 - sin2);
        float rs = (eta * facing - transmittedCos) / max(eta * facing + transmittedCos, 1e-4);
        float rp = (facing - eta * transmittedCos) / max(facing + eta * transmittedCos, 1e-4);
        fresnel = 0.5 * (rs * rs + rp * rp);
    }

    // The ripples bend the view of what lies below.
    // Refraction fades out with distance so far water never shifts across the horizon.
    vec2 screen = gl_FragCoord.xy / vec2(textureSize(uSceneColor, 0));
    float refrFade = 1.0 - smoothstep(5.0, 35.0, distance);
    vec4 surfaceClip = frame.viewProjection * vec4(vView, 1.0);
    float straightDepth = sceneDepth(screen);
    float gap = max(viewDepth(straightDepth) - surfaceClip.w, 0.0);
    vec4 projectedSlope = frame.viewProjection * vec4(normal - faceNormal, 0.0);
    vec2 offset = projectedSlope.xy * (0.15 * refrFade / max(surfaceClip.w, 1.0));
    offset *= smoothstep(0.0, 0.5, gap);
    if (frame.fog.z > 0.5) {
        offset += underwaterRipple(screen);
    }
    vec2 bent = sceneUv(screen + offset * sceneContinuity(screen));
    float floorDepth = sceneDepth(bent);
    // If the bent lookup lands on something in front of the water or crosses onto the sky,
    // fall back to the straight screen coordinate.
    if (viewDepth(floorDepth) <= surfaceClip.w || (floorDepth >= 1.0 && straightDepth < 1.0)) {
        bent = sceneUv(screen);
        floorDepth = straightDepth;
    }
    vec3 behind = frame.uiState.x > 0.5
        ? blurSceneColor(bent)
        : textureLod(uSceneColor, bent, 0.0).rgb;

    // Light from the floor fades with the distance it travels through the
    // water (Beer-Lambert), so shallow water is clear and deep water shows
    // its own scattered tone.
    vec3 floorPoint = scenePosition(bent, floorDepth);
    float thickness = floorDepth >= 1.0 ? 32.0 : max(length(floorPoint) - distance, 0.0);
    float transmit = fromBelow ? 1.0 : exp(-WATER_ABSORPTION * thickness);
    vec3 scatter = WATER_COLOR * vTint
        * (SKY_AMBIENT + SUN_COLOR * max(sun.y, 0.0) * 0.3 * lit);

    vec3 reflected = reflect(viewDir, n);
    vec3 reflection = tonemap(fromBelow
        ? WATER_FOG
        : skyColor(reflected) + SUN_COLOR * 8.0 * pow(max(dot(reflected, sun), 0.0), 600.0) * lit);
    if (!fromBelow && frame.fog.w > 0.5) {
        vec3 hit = reflected.y > 0.0
            ? traceReflection(vView + faceNormal * 0.05, reflected, 1.0)
            : vec3(0.0);
        vec3 hitCol = frame.uiState.x > 0.5
            ? blurSceneColor(hit.xy)
            : textureLod(uSceneColor, hit.xy, 0.0).rgb;
        reflection = mix(reflection, hitCol, hit.z);
    }

    if (fromBelow) {
        vec3 transmitted = refract(viewDir, n, eta);
        vec3 outside = tonemap(skyColor(transmitted)
            + SUN_COLOR * 20.0 * smoothstep(0.9995, 0.9998, dot(transmitted, sun)));
        float surfaceY = frame.cameraPosition.y + vView.y;
        // Only geometry on the air side belongs in the transmitted view.
        if (floorDepth >= 1.0 || frame.cameraPosition.y + floorPoint.y < surfaceY) {
            behind = outside;
        }
    }

    // The scene is already tone mapped, so everything is mixed in display
    // range, as blending over it would.
    vec3 color = mix(behind, tonemap(scatter), 1.0 - transmit);
    color = mix(color, reflection, fresnel);

    vec3 finalColor;
    if (frame.fog.z > 0.5) {
        finalColor = underwaterColor(color, viewDir, distance, 0.5);
    } else {
        vec4 fog = fogBetween(viewDir, distance);
        finalColor = mix(color, tonemap(fog.rgb), fog.a);
    }
    if (frame.uiState.x > 0.5) {
        finalColor *= sceneVignette(screen);
    }
    outColor = vec4(finalColor, 1.0);
}
