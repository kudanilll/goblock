#version 450

// Fullscreen triangle built from gl_VertexIndex, so the draw needs no vertex
// buffer.
layout(location = 0) out vec2 vUV;

void main() {
    vec2 pos = vec2((gl_VertexIndex << 1) & 2, gl_VertexIndex & 2);
    gl_Position = vec4(pos * 2.0 - 1.0, 0.0, 1.0);
    // Vulkan NDC has Y pointing down. Flip v so 0 stays at the bottom.
    vUV = vec2(pos.x, 1.0 - pos.y);
}
