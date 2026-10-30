#version 450
#extension GL_GOOGLE_include_directive : require

#include "common.glsl"
#include "scene.glsl"

// Copies the scene drawn so far onto the swapchain image. When in the settings menu,
// it blurs and dims the background. When underwater, it adds ripple and godrays.
layout(location = 0) out vec4 outColor;

vec3 blurScene(vec2 uv) {
    vec2 pixel = 1.0 / vec2(textureSize(uSceneColor, 0));
    float r = 3.5;
    vec3 col = vec3(0.0);
    col += textureLod(uSceneColor, sceneUv(uv + vec2(-r, -r) * pixel), 0.0).rgb * 0.09;
    col += textureLod(uSceneColor, sceneUv(uv + vec2(0.0, -r) * pixel), 0.0).rgb * 0.12;
    col += textureLod(uSceneColor, sceneUv(uv + vec2(r, -r) * pixel), 0.0).rgb * 0.09;
    col += textureLod(uSceneColor, sceneUv(uv + vec2(-r, 0.0) * pixel), 0.0).rgb * 0.12;
    col += textureLod(uSceneColor, sceneUv(uv), 0.0).rgb * 0.16;
    col += textureLod(uSceneColor, sceneUv(uv + vec2(r, 0.0) * pixel), 0.0).rgb * 0.12;
    col += textureLod(uSceneColor, sceneUv(uv + vec2(-r, r) * pixel), 0.0).rgb * 0.09;
    col += textureLod(uSceneColor, sceneUv(uv + vec2(0.0, r) * pixel), 0.0).rgb * 0.12;
    col += textureLod(uSceneColor, sceneUv(uv + vec2(r, r) * pixel), 0.0).rgb * 0.09;

    vec2 d = uv * 2.0 - 1.0;
    float vignette = clamp(1.0 - dot(d, d) * 0.20, 0.0, 1.0);
    return col * (0.58 * vignette);
}

void main() {
    vec2 screenSize = vec2(textureSize(uSceneColor, 0));
    vec2 screenUv = gl_FragCoord.xy / screenSize;

    if (frame.uiState.x > 0.5) {
        outColor = vec4(blurScene(screenUv), 1.0);
        return;
    }

    if (frame.fog.z > 0.5) {
        vec2 distortedUv = sceneUv(screenUv + underwaterRipple(screenUv) * sceneContinuity(screenUv));
        vec3 baseColor = textureLod(uSceneColor, distortedUv, 0.0).rgb;
        vec3 point = scenePosition(distortedUv, sceneDepth(distortedUv));
        float distance = length(point);
        outColor = vec4(underwaterColor(baseColor, point / max(distance, 1e-4), distance, 0.5), 1.0);
    } else {
        outColor = vec4(texelFetch(uSceneColor, ivec2(gl_FragCoord.xy), 0).rgb, 1.0);
    }
}
