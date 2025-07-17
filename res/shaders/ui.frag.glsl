out vec4 FragColor;

in vec2 TexCoord;

uniform sampler2D tex;

void main() {
    vec4 texColor = vec4(texture(tex, TexCoord));
    if (texColor.a < 0.1) {
        discard;
    }
    FragColor = texColor;
}