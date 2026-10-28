#version 330 core
in vec3 vColor;
in vec3 vNormal;

out vec4 FragColor;

uniform vec3 lightDir;

void main() {
    // Simple directional shading so block faces read as separate surfaces.
    float diffuse = max(dot(normalize(vNormal), normalize(-lightDir)), 0.0);
    vec3 color = vColor * (0.55 + 0.45 * diffuse);
    FragColor = vec4(color, 1.0);
}
