layout (location = 0) in vec2 aPos;
layout (location = 1) in vec2 aTexCoord;

out vec2 TexCoord;

uniform vec2 screenSize;
uniform vec2 atlasSize;

void main() {
    gl_Position = vec4(2.0 * ((aPos / screenSize) - vec2(0.5)), 0.0, 1.0);
    TexCoord = aTexCoord / atlasSize;
}