#version 450

// Copies the scene drawn so far onto the swapchain image, so the water can
// be drawn over it. The scene image has the same size as the screen.
layout(set = 0, binding = 3) uniform sampler2D uSceneColor;

layout(location = 0) out vec4 outColor;

void main() {
    outColor = vec4(texelFetch(uSceneColor, ivec2(gl_FragCoord.xy), 0).rgb, 1.0);
}
