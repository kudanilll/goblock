#version 330 core
out vec4 FragColor;
in vec2 TexCoords;
uniform float time;

void main() {
    // Warna langit siang yang cerah
    vec3 topColor = vec3(0.2, 0.4, 0.8);     // Biru tua
    vec3 middleColor = vec3(0.53, 0.81, 0.98);  // Biru cerah
    vec3 bottomColor = vec3(0.7, 0.87, 1.0);    // Biru muda
    
    float y = TexCoords.y;
    vec3 finalColor;
    
    if (y < 0.5) {
        finalColor = mix(bottomColor, middleColor, y * 2.0);
    } else {
        finalColor = mix(middleColor, topColor, (y - 0.5) * 2.0);
    }
    
    // Tambah sedikit awan bergerak
    float cloud = sin(TexCoords.x * 10.0 + time * 0.1) * 0.05;
    finalColor += vec3(cloud);
    
    FragColor = vec4(finalColor, 1.0);
}