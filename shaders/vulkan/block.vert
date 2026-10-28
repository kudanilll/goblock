#version 450

layout(location = 0) in vec3 aPos;
layout(location = 1) in vec3 aNormal;
layout(location = 2) in vec3 aColor;

// Shared with block.frag. Keep the block within 128 bytes, the most Vulkan
// 1.0 guarantees for push constants.
layout(push_constant) uniform Push {
    mat4 viewProjection;
    vec3 lightDir;
} pc;

layout(location = 0) out vec3 vColor;
layout(location = 1) out vec3 vNormal;

void main() {
    gl_Position = pc.viewProjection * vec4(aPos, 1.0);
    vColor = aColor;
    vNormal = aNormal;
}
