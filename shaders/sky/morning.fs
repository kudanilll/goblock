#version 330 core
out vec4 FragColor;
in vec2 TexCoords;
uniform float time; // Waktu untuk animasi

void main() {
    // Gradien langit pagi
    vec3 topColor = vec3(0.3, 0.5, 0.85);    // Biru tua
    vec3 middleColor = vec3(0.6, 0.8, 0.9);  // Biru muda
    vec3 bottomColor = vec3(0.9, 0.7, 0.5);  // Oranye lembut
    
    float y = TexCoords.y;
    vec3 finalColor;
    
    if (y < 0.5) {
        finalColor = mix(bottomColor, middleColor, y * 2.0);
    } else {
        finalColor = mix(middleColor, topColor, (y - 0.5) * 2.0);
    }
    
    // Tambah efek cahaya pagi
    float sunEffect = pow(1.0 - distance(TexCoords, vec2(0.8, 0.2)), 2.0);
    finalColor += vec3(0.2, 0.1, 0.0) * sunEffect;
    
    FragColor = vec4(finalColor, 1.0);
}