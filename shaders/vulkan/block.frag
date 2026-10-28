#version 450

layout(location = 0) in vec3 vColor;
layout(location = 1) in vec3 vNormal;

layout(push_constant) uniform Push {
    mat4 viewProjection;
    vec3 lightDir;
} pc;

layout(location = 0) out vec4 outColor;

void main() {
    float diffuse = max(dot(normalize(vNormal), normalize(-pc.lightDir)), 0.0);
    outColor = vec4(vColor * (0.55 + 0.45 * diffuse), 1.0);
}
