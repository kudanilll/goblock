#version 450

// Fullscreen triangle built from gl_VertexIndex, so the draw needs no vertex
// buffer.
layout(location = 0) out vec2 vNdc;

void main() {
    vec2 pos = vec2((gl_VertexIndex << 1) & 2, gl_VertexIndex & 2);
    vNdc = pos * 2.0 - 1.0;
    gl_Position = vec4(vNdc, 0.0, 1.0);
}
