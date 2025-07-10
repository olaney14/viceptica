out vec4 FragColor;

in vec2 TexCoord;

uniform sampler2D atlas;

void main() {
    vec4 texColor = vec4(texture(atlas, TexCoord));
    if (texColor.a < 0.1) {
        discard;
    }
    FragColor = texColor;
    //FragColor = vec4(0.0, 0.0, 0.0, 1.0);
}