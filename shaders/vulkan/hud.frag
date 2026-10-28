#version 450

layout(location = 0) in vec2 vUV;

layout(set = 0, binding = 0) uniform sampler2D uAtlas;

layout(push_constant) uniform Push {
    vec2 screenSize;
    vec4 color;
} pc;

layout(location = 0) out vec4 outColor;

void main() {
    float alpha = texture(uAtlas, vUV).r;
    outColor = vec4(pc.color.rgb, pc.color.a * alpha);
}
