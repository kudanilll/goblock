#version 450

layout(location = 0) in vec2 aPos;
layout(location = 1) in vec2 aUV;

layout(push_constant) uniform Push {
    vec2 screenSize;
    vec4 color;
} pc;

layout(location = 0) out vec2 vUV;

void main() {
    // aPos is in pixels with the origin at the top-left. Vulkan NDC also has
    // Y pointing down, so screen Y maps across without a flip.
    vec2 ndc = vec2(
        (aPos.x / pc.screenSize.x) * 2.0 - 1.0,
        (aPos.y / pc.screenSize.y) * 2.0 - 1.0
    );
    gl_Position = vec4(ndc, 0.0, 1.0);
    vUV = aUV;
}
