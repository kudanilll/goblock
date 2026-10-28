#version 330 core
out vec4 FragColor;
in vec2 TexCoords;
uniform float time;

void main() {
    // Warna sunset yang dramatis
    vec3 topColor = vec3(0.2, 0.2, 0.6);     // Biru gelap
    vec3 middleColor = vec3(0.8, 0.4, 0.2);  // Oranye
    vec3 bottomColor = vec3(0.9, 0.6, 0.3);  // Kuning keemasan
    
    float y = TexCoords.y;
    vec3 finalColor;
    
    if (y < 0.5) {
        finalColor = mix(bottomColor, middleColor, y * 2.0);
    } else {
        finalColor = mix(middleColor, topColor, (y - 0.5) * 2.0);
    }
    
    // Efek matahari terbenam
    float sunsetGlow = pow(1.0 - distance(TexCoords, vec2(0.5, 0.2)), 3.0);
    finalColor += vec3(0.3, 0.1, 0.0) * sunsetGlow;
    
    FragColor = vec4(finalColor, 1.0);
}