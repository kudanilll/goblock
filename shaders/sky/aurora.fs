#version 330 core
out vec4 FragColor;
in vec2 TexCoords;
uniform float time;

void main() {
    // Warna dasar langit malam
    vec3 topColor = vec3(0.0, 0.0, 0.1);
    vec3 middleColor = vec3(0.05, 0.05, 0.15);
    vec3 bottomColor = vec3(0.1, 0.1, 0.2);
    
    float y = TexCoords.y;
    vec3 finalColor;
    
    if (y < 0.5) {
        finalColor = mix(bottomColor, middleColor, y * 2.0);
    } else {
        finalColor = mix(middleColor, topColor, (y - 0.5) * 2.0);
    }
    
    // Efek aurora
    float aurora = sin(TexCoords.x * 5.0 + time) * sin(TexCoords.y * 5.0 + time * 0.5);
    aurora *= pow(TexCoords.y, 2.0); // Aurora lebih kuat di atas
    
    vec3 auroraColor = vec3(0.0, 1.0, 0.5); // Warna hijau aurora
    finalColor += auroraColor * aurora * 0.5;
    
    // Tambah bintang
    float stars = pow(fract(sin(TexCoords.x * 100.0 + TexCoords.y * 100.0) * 43758.5453123), 20.0);
    finalColor += vec3(stars * 0.3);
    
    FragColor = vec4(finalColor, 1.0);
}