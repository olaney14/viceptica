out vec4 FragColor;

uniform vec3 color;

void main() {
    FragColor = vec4(color.rgb, 1.0f);
}