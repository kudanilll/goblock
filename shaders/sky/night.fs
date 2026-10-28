#version 330 core
out vec4 FragColor;
in vec2 TexCoords;
uniform float time;

void main() {
    // Warna langit malam
    vec3 topColor = vec3(0.0, 0.0, 0.2);        // Biru sangat gelap
    vec3 middleColor = vec3(0.05, 0.05, 0.15);  // Biru gelap
    vec3 bottomColor = vec3(0.1, 0.1, 0.3);     // Sedikit lebih terang
    
    // Gradasi langit
    float y = TexCoords.y;
    vec3 skyColor = mix(bottomColor, middleColor, smoothstep(0.0, 0.5, y));
    skyColor = mix(skyColor, topColor, smoothstep(0.5, 1.0, y));
    
    // Bintang
    float starSeed = fract(sin(dot(TexCoords * 100.0, vec2(12.9898, 78.233))) * 43758.5453123);
    float starBrightness = pow(starSeed, 30.0);
    float starSize = step(0.995, starSeed);
    vec3 starColor = vec3(1.0, 1.0, 0.9) * starBrightness * starSize;

    // Awan
    float cloudNoise = fract(sin(dot(TexCoords * vec2(80.0, 40.0) + time * 0.1, vec2(12.9898, 78.233))) * 43758.5453123);
    float cloudOpacity = smoothstep(0.3, 0.7, cloudNoise) * 0.1;
    vec3 cloudColor = vec3(0.2, 0.2, 0.25) * cloudOpacity;

    vec3 finalColor = skyColor + starColor + cloudColor;
    FragColor = vec4(finalColor, 1.0);
}
