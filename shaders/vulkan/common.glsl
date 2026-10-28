// Shared by the world shaders: the per-frame uniform block, the sky model,
// and tone mapping. glslc resolves the #include at build time.
//
// Everything is in linear light with values above 1 allowed; tonemap()
// brings the result into display range at the very end.

layout(set = 0, binding = 0) uniform Frame {
    // Camera at the origin; positions are relative to the camera.
    mat4 viewProjection;
    mat4 inverseViewProjection;
    // Camera-relative position to shadow map clip space.
    mat4 shadowMatrix;
    // xyz: camera position in the world. w: seconds since start.
    vec4 cameraPosition;
    // xyz: direction toward the sun. w: shadow map texel size, 0 when off.
    vec4 sunDirection;
    // x: distance where fog starts, y: distance where it hides everything.
    // z: 1 when the camera is under water. w: 1 when water traces
    // screen-space reflections.
    vec4 fog;
    // xyz: player's feet relative to the camera. w: body height.
    vec4 playerFeet;
} frame;

layout(set = 0, binding = 2) uniform sampler2DShadow uShadow;

// What the world fades into when seen from under water.
const vec3 WATER_FOG = vec3(0.04, 0.16, 0.20);

const vec3 SUN_COLOR = vec3(1.0, 0.93, 0.82) * 3.0;
const vec3 SKY_ZENITH = vec3(0.10, 0.25, 0.65);
const vec3 SKY_HORIZON = vec3(0.62, 0.74, 0.90);
// Light reaching a surface from the open sky above it.
const vec3 SKY_AMBIENT = vec3(0.32, 0.42, 0.58);
// Light bounced off the ground, reaching surfaces that face down.
const vec3 GROUND_BOUNCE = vec3(0.16, 0.14, 0.11);

// Block faces follow goblock_world::Face::ALL, then the two grass diagonals.
const vec3 FACE_NORMALS[8] = vec3[](
    vec3(0.0, 1.0, 0.0),
    vec3(0.0, -1.0, 0.0),
    vec3(0.0, 0.0, -1.0),
    vec3(0.0, 0.0, 1.0),
    vec3(-1.0, 0.0, 0.0),
    vec3(1.0, 0.0, 0.0),
    vec3(-0.70710678, 0.0, 0.70710678),
    vec3(-0.70710678, 0.0, -0.70710678)
);

// Shared by the scene and shadow passes so moving foliage keeps its shadow.
vec3 vegetationPosition(vec3 view, float weight) {
    if (weight == 0.0) {
        return view;
    }
    vec3 world = frame.cameraPosition.xyz + view;
    float time = frame.cameraPosition.w;
    float breeze = sin(dot(world.xz, vec2(0.23, 0.17)) + time * 1.7) * 0.18
                 + sin(dot(world.xz, vec2(-0.41, 0.29)) + time * 2.9) * 0.06;
    vec2 wind = vec2(0.91, 0.41) * breeze * weight;

    // Leaf weights stay below 0.5; only grass tips react to the body.
    if (weight > 0.5) {
        vec2 away = view.xz - frame.playerFeet.xz;
        float distance = length(away);
        float aboveFeet = view.y - frame.playerFeet.y;
        // Tips can reach the next cell boundary, but roots stay in the cell below.
        float rootAboveFeet = floor(world.y - 1.0 / 32.0)
                            - frame.cameraPosition.y - frame.playerFeet.y;
        float vertical = smoothstep(-0.1, 0.15, aboveFeet)
                       * (1.0 - smoothstep(frame.playerFeet.w, frame.playerFeet.w + 0.25, rootAboveFeet));
        float bend = (1.0 - smoothstep(0.2, 1.15, distance)) * vertical;
        view.xz += away / max(distance, 0.05) * bend * 0.48;
        view.y -= bend * 0.22;
    }
    view.xz += wind;
    return view;
}

// Colour of the sky in direction `dir`.
vec3 skyColor(vec3 dir) {
    // Straight up, light has crossed little air and scattered blue dominates.
    // Toward the horizon the path through the air is long and the sky pales.
    float up = clamp(dir.y, 0.0, 1.0);
    vec3 sky = mix(SKY_HORIZON, SKY_ZENITH, sqrt(up));

    // Haze scatters light forward, so the sky brightens around the sun.
    float toSun = max(dot(dir, frame.sunDirection.xyz), 0.0);
    sky += SUN_COLOR * (0.06 * pow(toSun, 6.0) + 0.25 * pow(toSun, 64.0));

    // Below the horizon the haze thickens toward a darker ground colour, so
    // distant terrain fogs into the same colour the sky shows there.
    if (dir.y < 0.0) {
        sky = mix(sky, SKY_HORIZON * 0.75, clamp(-dir.y * 4.0, 0.0, 1.0));
    }
    return sky;
}

// Filmic curve from Krzysztof Narkowicz's fit of the ACES tone mapper.
vec3 tonemap(vec3 color) {
    return clamp((color * (2.51 * color + 0.03)) / (color * (2.43 * color + 0.59) + 0.14), 0.0, 1.0);
}

// Fraction of sunlight reaching the shadow map position `coord`, from a
// 3x3 block of depth comparisons that softens the shadow's edge.
float sunlight(vec3 coord) {
    float texel = frame.sunDirection.w;
    if (texel == 0.0) {
        return 1.0;
    }
    float lit = 0.0;
    for (int y = -1; y <= 1; y++) {
        for (int x = -1; x <= 1; x++) {
            lit += texture(uShadow, vec3(coord.xy + vec2(x, y) * texel, min(coord.z, 1.0)));
        }
    }
    return lit / 9.0;
}

// The fog between the camera and a point `distance` away in direction
// `dir`: its colour in rgb and how much it hides in a.
vec4 fogBetween(vec3 dir, float distance) {
    if (frame.fog.z > 0.5) {
        // Under water, light dies out within a few blocks.
        return vec4(WATER_FOG, 1.0 - exp(-distance * 0.12));
    }
    // Aerial perspective: distant terrain takes on the colour of the sky
    // behind it. A light haze starts right away, and the fog closes
    // completely before the edge of the loaded world.
    float haze = 1.0 - exp(-distance * 0.002);
    float fog = max(smoothstep(frame.fog.x, frame.fog.y, distance), haze * 0.5);
    return vec4(skyColor(dir), fog);
}
