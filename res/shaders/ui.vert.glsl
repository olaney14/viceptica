// lmao
const vec2 vertices[4] = vec2[]
(
    vec2(0.0, 1.0),
    vec2(0.0, 0.0),
    vec2(1.0, 1.0),
    vec2(1.0, 0.0)
);

out vec2 TexCoord;

// these are all in px
uniform vec2 screenSize;
uniform vec2 texSize;
uniform vec2 pos;
uniform vec2 scale;
uniform vec2 texturePos;
uniform vec2 textureScale;

uniform float z;

void main() {
    vec2 finalPos = 2.0 * ((pos / screenSize) - vec2(0.5)) * vec2(1.0, -1.0);
    vec2 finalScale = 2.0 * (scale / screenSize) * vec2(1.0, -1.0);

    gl_Position = vec4(finalPos + finalScale * vertices[gl_VertexID], -z, 1.0);

    vec2 txPos = (texturePos / texSize) * vec2(1.0, -1.0);
    vec2 txScale = (textureScale / texSize) * vec2(1.0, -1.0);

    TexCoord = txPos + txScale * vertices[gl_VertexID];
}