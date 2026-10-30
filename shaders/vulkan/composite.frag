#version 450
#extension GL_GOOGLE_include_directive : require

#include "common.glsl"
#include "scene.glsl"

// Copies the scene drawn so far onto the swapchain image. When underwater,
// it adds a subtle optical ripple and volumetric sunlight godrays.
layout(location = 0) out vec4 outColor;

void main() {
    vec2 screenSize = vec2(textureSize(uSceneColor, 0));
    vec2 screenUv = gl_FragCoord.xy / screenSize;

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
