#version 450
#extension GL_GOOGLE_include_directive : require

#include "common.glsl"

layout(location = 0) in vec2 vNdc;

layout(location = 0) out vec4 outColor;
// The depth copy: nothing but sky here.
layout(location = 1) out float outDepth;

void main() {
    if (frame.fog.z > 0.5) {
        outColor = vec4(tonemap(WATER_FOG), 1.0);
        outDepth = 1.0;
        return;
    }

    // The point on the far plane behind this pixel, relative to the camera.
    vec4 far = frame.inverseViewProjection * vec4(vNdc, 1.0, 1.0);
    vec3 dir = normalize(far.xyz / far.w);

    vec3 color = skyColor(dir);
    float toSun = dot(dir, frame.sunDirection.xyz);
    color += SUN_COLOR * 20.0 * smoothstep(0.9995, 0.9998, toSun);
    outColor = vec4(tonemap(color), 1.0);
    outDepth = 1.0;
}
