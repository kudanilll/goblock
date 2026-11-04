#version 450
#extension GL_GOOGLE_include_directive : require

#include "common.glsl"
#include "scene.glsl"

// Copies the scene drawn so far onto the swapchain image. When in the settings menu,
// it blurs and dims the background. When underwater, it adds ripple and godrays.
layout(location = 0) out vec4 outColor;

void main() {
    vec2 screenSize = vec2(textureSize(uSceneColor, 0));
    vec2 screenUv = gl_FragCoord.xy / screenSize;

    if (frame.uiState.x > 0.5) {
        outColor = vec4(blurScene(screenUv), 1.0);
        return;
    }

    if (frame.fog.z > 0.5) {
        vec2 distortedUv = sceneUv(screenUv + underwaterRipple(screenUv) * sceneContinuity(screenUv));
        float ao = 1.0;
        if (frame.uiState.y > 0.5) {
            ao = evaluateSSAO(distortedUv, gl_FragCoord.xy);
        }
        vec3 baseColor = textureLod(uSceneColor, distortedUv, 0.0).rgb * ao;
        vec3 point = scenePosition(distortedUv, sceneDepth(distortedUv));
        float distance = length(point);
        outColor = vec4(underwaterColor(baseColor, point / max(distance, 1e-4), distance, 0.5), 1.0);
    } else {
        float ao = 1.0;
        if (frame.uiState.y > 0.5) {
            ao = evaluateSSAO(screenUv, gl_FragCoord.xy);
        }
        vec3 baseColor = texelFetch(uSceneColor, ivec2(gl_FragCoord.xy), 0).rgb * ao;
        outColor = vec4(baseColor, 1.0);
    }
}
