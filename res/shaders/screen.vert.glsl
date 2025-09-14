const vec2 vertices[4] = vec2[]
(
    vec2(-1.0,  1.0),
    vec2(-1.0, -1.0),
    vec2( 1.0,  1.0),
    vec2( 1.0, -1.0)
);
const vec2 texCoords[4] = vec2[]
(
    vec2(0.0, 1.0),
    vec2(0.0, 0.0),
    vec2(1.0, 1.0),
    vec2(1.0, 0.0)
);

out vec2 TexCoord;

void main() {
    gl_Position = vec4(vertices[gl_VertexID], 0.0, 1.0);
    TexCoord = texCoords[gl_VertexID];
    // vec2 finalPos = 2.0 * ((pos / screenSize) - vec2(0.5)) * vec2(1.0, -1.0);
    // vec2 finalScale = 2.0 * (scale / screenSize) * vec2(1.0, -1.0);

    // gl_Position = vec4(finalPos + finalScale * vertices[gl_VertexID], 0.0, 1.0);

    // vec2 txPos = (texturePos / texSize) * vec2(1.0, -1.0);
    // vec2 txScale = (textureScale / texSize) * vec2(1.0, -1.0);

    // TexCoord = txPos + txScale * vertices[gl_VertexID];
}