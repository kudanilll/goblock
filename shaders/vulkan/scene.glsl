layout(set = 0, binding = 3) uniform sampler2D uSceneColor;
layout(set = 0, binding = 4) uniform sampler2D uSceneDepth;

vec2 sceneUv(vec2 uv) {
    vec2 halfPixel = 0.5 / vec2(textureSize(uSceneDepth, 0));
    return clamp(uv, halfPixel, 1.0 - halfPixel);
}

float sceneDepth(vec2 uv) {
    return textureLod(uSceneDepth, sceneUv(uv), 0.0).r;
}

vec3 scenePosition(vec2 uv, float depth) {
    vec4 p = frame.inverseViewProjection * vec4(uv * 2.0 - 1.0, depth, 1.0);
    return p.xyz / p.w;
}

float viewDepth(float depth) {
    return 1.0 / (frame.inverseViewProjection[2].w * depth
                + frame.inverseViewProjection[3].w);
}

// MSAA resolves mix foreground and background depth at silhouettes.
// Such pixels cannot establish a reliable reflection or refraction hit.
float sceneContinuity(vec2 uv) {
    vec2 pixel = 1.0 / vec2(textureSize(uSceneDepth, 0));
    float centre = viewDepth(sceneDepth(uv));
    vec4 around = vec4(viewDepth(sceneDepth(uv + vec2(pixel.x, 0.0))),
                       viewDepth(sceneDepth(uv - vec2(pixel.x, 0.0))),
                       viewDepth(sceneDepth(uv + vec2(0.0, pixel.y))),
                       viewDepth(sceneDepth(uv - vec2(0.0, pixel.y))));
    vec4 delta = abs(around - centre);
    float spread = max(max(delta.x, delta.y), max(delta.z, delta.w));
    return 1.0 - smoothstep(0.05, 0.2, spread / max(centre, 0.1));
}

vec3 blurSceneColor(vec2 uv) {
    vec2 pixel = 1.0 / vec2(textureSize(uSceneColor, 0));
    float r = 3.5;
    vec3 col = vec3(0.0);
    col += textureLod(uSceneColor, sceneUv(uv + vec2(-r, -r) * pixel), 0.0).rgb * 0.09;
    col += textureLod(uSceneColor, sceneUv(uv + vec2(0.0, -r) * pixel), 0.0).rgb * 0.12;
    col += textureLod(uSceneColor, sceneUv(uv + vec2(r, -r) * pixel), 0.0).rgb * 0.09;
    col += textureLod(uSceneColor, sceneUv(uv + vec2(-r, 0.0) * pixel), 0.0).rgb * 0.12;
    col += textureLod(uSceneColor, sceneUv(uv), 0.0).rgb * 0.16;
    col += textureLod(uSceneColor, sceneUv(uv + vec2(r, 0.0) * pixel), 0.0).rgb * 0.12;
    col += textureLod(uSceneColor, sceneUv(uv + vec2(-r, r) * pixel), 0.0).rgb * 0.09;
    col += textureLod(uSceneColor, sceneUv(uv + vec2(0.0, r) * pixel), 0.0).rgb * 0.12;
    col += textureLod(uSceneColor, sceneUv(uv + vec2(r, r) * pixel), 0.0).rgb * 0.09;
    return col;
}

float sceneVignette(vec2 uv) {
    vec2 d = uv * 2.0 - 1.0;
    return clamp(1.0 - dot(d, d) * 0.20, 0.0, 1.0) * 0.58;
}

vec3 blurScene(vec2 uv) {
    return blurSceneColor(uv) * sceneVignette(uv);
}
