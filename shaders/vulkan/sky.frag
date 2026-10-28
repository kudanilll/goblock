#version 450

layout(location = 0) in vec2 vNdc;

layout(push_constant) uniform Push {
    mat4 inverseViewProjection;
    vec4 zenith;
    vec4 horizon;
    vec4 sunDirection;
} pc;

layout(location = 0) out vec4 outColor;

void main() {
    // The point on the far plane behind this pixel, relative to the camera.
    vec4 far = pc.inverseViewProjection * vec4(vNdc, 1.0, 1.0);
    vec3 dir = normalize(far.xyz / far.w);

    // Below the horizon the sky keeps the horizon colour, which is also the
    // fog colour, so distant terrain melts into it.
    float height = clamp(dir.y, 0.0, 1.0);
    vec3 color = mix(pc.horizon.rgb, pc.zenith.rgb, sqrt(height));

    float sun = max(dot(dir, pc.sunDirection.xyz), 0.0);
    color += vec3(1.0, 0.92, 0.75) * (pow(sun, 512.0) * 3.0 + pow(sun, 12.0) * 0.12);
    outColor = vec4(color, 1.0);
}
