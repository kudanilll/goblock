#version 450

layout(location = 0) in vec2 vNdc;

layout(set = 0, binding = 0) uniform sampler2DArray uBackgrounds;

layout(push_constant) uniform Push {
    // x: layer A index, y: layer B index, z: crossfade blend (0..1), w: unused
    vec4 params;
} pc;

layout(location = 0) out vec4 outColor;

void main() {
    vec2 uv = vNdc * 0.5 + 0.5;
    vec3 colA = texture(uBackgrounds, vec3(uv, pc.params.x)).rgb;
    vec3 colB = texture(uBackgrounds, vec3(uv, pc.params.y)).rgb;
    vec3 col = mix(colA, colB, clamp(pc.params.z, 0.0, 1.0));

    // Subtle vignette
    vec2 d = vNdc;
    float vignette = clamp(1.0 - dot(d, d) * 0.15, 0.0, 1.0);
    outColor = vec4(col * vignette, 1.0);
}
