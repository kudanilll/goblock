#version 450

layout(location = 0) in vec2 vUV;

layout(push_constant) uniform Push {
    float time;
} pc;

layout(location = 0) out vec4 outColor;

void main() {
    vec3 topColor = vec3(0.20, 0.20, 0.60);
    vec3 middleColor = vec3(0.80, 0.40, 0.20);
    vec3 bottomColor = vec3(0.90, 0.60, 0.30);

    float y = vUV.y;
    vec3 color;
    if (y < 0.5) {
        color = mix(bottomColor, middleColor, y * 2.0);
    } else {
        color = mix(middleColor, topColor, (y - 0.5) * 2.0);
    }

    float glow = pow(max(1.0 - distance(vUV, vec2(0.5, 0.2)), 0.0), 3.0);
    color += vec3(0.3, 0.1, 0.0) * glow;

    outColor = vec4(color, 1.0);
}
