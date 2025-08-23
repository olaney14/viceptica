layout (location = 0) in vec3 aPos;
layout (location = 1) in vec3 aColor;
layout (location = 2) in vec2 aTexCoord;
layout (location = 3) in vec3 aNormal;

out vec3 vertexColor;
out vec2 TexCoord;
flat out uint fullbright;
flat out uint cutout;
out vec3 normal;
out vec3 fragPos;

uniform int flags;
uniform mat4 model;
uniform mat3 normal_matrix;
uniform mat4 view;
uniform mat4 projection;

const float TEXTURE_LOOP_DIV = 2.0f;

void main() {
    gl_Position = projection * view * model * vec4(aPos, 1.0);
    vertexColor = aColor;
    uint extend_texture = flags & 1;
    fullbright = flags & 2;
    // skip is unused for this shader
    cutout = flags & 8;
    
    fragPos = vec3(model * vec4(aPos, 1.0));
    normal = normal_matrix * aNormal;
    // normal = mat3(transpose(inverse(model))) * aNormal;

    // Texture coordinates
    if (extend_texture > 0) { // Loop texture
        // this is wrong fix it later
        // vec3 scaledNormal = normalize((model * vec4(normal, 0.0)).xyz);
        float dotY = abs(dot(vec3(0.0f, 1.0f, 0.0f), normal));
        float dotX = abs(dot(vec3(1.0f, 0.0f, 0.0f), normal));
        float dotZ = abs(dot(vec3(0.0f, 0.0f, 1.0f), normal));
        
        if (dotY > dotX && dotY > dotZ) {
            TexCoord = fragPos.xz / TEXTURE_LOOP_DIV;
        } else if (dotX > dotZ && dotX > dotY) {
            TexCoord = fragPos.zy / TEXTURE_LOOP_DIV;
        } else {
            TexCoord = fragPos.xy / TEXTURE_LOOP_DIV;
        }
    } else {
        TexCoord = aTexCoord;
    }
}