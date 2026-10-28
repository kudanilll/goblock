#version 450

// Set per pipeline: 0 opaque, 1 cutout (drops transparent texels),
// 2 translucent (alpha blended).
layout(constant_id = 0) const int ALPHA_MODE = 0;

layout(location = 0) in vec3 vUv;
layout(location = 1) in vec3 vColor;
layout(location = 2) in vec3 vView;

layout(set = 0, binding = 0) uniform sampler2DArray uBlocks;

layout(push_constant) uniform Push {
    mat4 viewProjection;
    vec4 chunkOffset;
    vec4 fogColor;
    vec4 fog;
} pc;

layout(location = 0) out vec4 outColor;

void main() {
    vec4 texel = texture(uBlocks, vUv);
    if (ALPHA_MODE == 1 && texel.a < 0.5) {
        discard;
    }
    float fog = smoothstep(pc.fog.x, pc.fog.y, length(vView));
    vec3 color = mix(texel.rgb * vColor, pc.fogColor.rgb, fog);
    float alpha = ALPHA_MODE == 2 ? texel.a : 1.0;
    outColor = vec4(color, alpha);
}
