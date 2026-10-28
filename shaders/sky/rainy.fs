#version 330 core
out vec4 FragColor;
in vec2 TexCoords;

void main() {
    // Warna langit abu-abu dengan gradasi
    vec3 topColor = vec3(0.3, 0.3, 0.3);     // Abu-abu gelap
    vec3 middleColor = vec3(0.5, 0.5, 0.5);  // Abu-abu sedang
    vec3 bottomColor = vec3(0.7, 0.7, 0.7);  // Abu-abu terang
    
    float y = TexCoords.y;
    vec3 finalColor;
    
    // Gradasi langit dari bawah ke atas
    finalColor = mix(bottomColor, middleColor, smoothstep(0.0, 0.5, y));
    finalColor = mix(finalColor, topColor, smoothstep(0.5, 1.0, y));
    
    FragColor = vec4(finalColor, 1.0);
}
