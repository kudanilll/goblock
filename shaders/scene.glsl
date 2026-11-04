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

// Interleaved gradient noise for jittering sample rotations per pixel.
float interleavedGradientNoise(vec2 screenPos) {
    return fract(52.9829189 * fract(dot(screenPos, vec2(0.06711056, 0.00583715))));
}

// 8 hemispherical Poisson spiral sample directions in tangent space (z pointing up).
const vec3 SSAO_SAMPLES[8] = vec3[](
    vec3( 0.35,  0.15, 0.30),
    vec3(-0.25,  0.35, 0.45),
    vec3( 0.20, -0.40, 0.50),
    vec3(-0.35, -0.20, 0.60),
    vec3( 0.50,  0.45, 0.40),
    vec3(-0.60, -0.30, 0.50),
    vec3( 0.30, -0.60, 0.55),
    vec3(-0.45,  0.55, 0.65)
);

float evaluateSSAO(vec2 uv, vec2 screenPos) {
    float depth = sceneDepth(uv);
    if (depth >= 0.9999) {
        return 1.0;
    }

    vec3 point = scenePosition(uv, depth);
    vec3 dPdx = dFdx(point);
    vec3 dPdy = dFdy(point);
    vec3 normal = cross(dPdx, dPdy);
    float normalLenSq = dot(normal, normal);
    if (normalLenSq < 1e-8) {
        return 1.0;
    }
    normal *= inversesqrt(normalLenSq);
    if (dot(normal, point) > 0.0) {
        normal = -normal;
    }

    vec3 up = abs(normal.y) < 0.95 ? vec3(0.0, 1.0, 0.0) : vec3(1.0, 0.0, 0.0);
    vec3 tangent = normalize(cross(up, normal));
    vec3 bitangent = cross(normal, tangent);

    float angle = interleavedGradientNoise(screenPos) * 6.2831853;
    float cosA = cos(angle);
    float sinA = sin(angle);

    float radius = 0.65;
    float occlusion = 0.0;

    for (int i = 0; i < 8; i++) {
        vec3 s = SSAO_SAMPLES[i];
        vec2 rot = vec2(s.x * cosA - s.y * sinA, s.x * sinA + s.y * cosA);
        vec3 sampleOffset = (tangent * rot.x + bitangent * rot.y + normal * s.z) * radius;
        vec3 samplePos = point + sampleOffset;

        vec4 clip = frame.viewProjection * vec4(samplePos, 1.0);
        if (clip.w <= 0.0) {
            continue;
        }

        vec2 sampleUv = (clip.xy / clip.w) * 0.5 + 0.5;
        if (sampleUv.x < 0.0 || sampleUv.x > 1.0 || sampleUv.y < 0.0 || sampleUv.y > 1.0) {
            continue;
        }

        float actualDepth = sceneDepth(sampleUv);
        if (actualDepth >= 0.9999) {
            continue;
        }

        float actualLinear = viewDepth(actualDepth);
        float sampleLinear = clip.w;

        float distDiff = sampleLinear - actualLinear;
        if (distDiff > 0.02 && distDiff < radius) {
            float rangeCheck = smoothstep(radius, 0.0, distDiff);
            occlusion += rangeCheck;
        }
    }

    float dist = length(point);
    float distanceFade = 1.0 - smoothstep(frame.fog.y * 0.25, frame.fog.y * 0.70, dist);
    float factor = (occlusion / 8.0) * distanceFade;

    return 1.0 - factor * 0.45;
}
