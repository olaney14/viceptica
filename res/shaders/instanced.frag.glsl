out vec4 FragColor;

in vec3 vertexColor;
in vec2 TexCoord;

uniform sampler2D textureIn;

void main() {
    FragColor = texture(textureIn, TexCoord) * vec4(vertexColor, 1.0f);
}