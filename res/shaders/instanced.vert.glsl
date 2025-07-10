layout (location = 0) in vec3 aPos;
layout (location = 1) in vec3 aColor;
layout (location = 2) in vec2 aTexCoord;
layout (location = 3) in vec3 aNormal;

layout (location = 4) in uint instanceFlags;
layout (location = 5) in mat4 instanceMatrix;

out vec3 vertexColor;
out vec2 TexCoord;
flat out uint fullbright;
out vec3 normal;
out vec3 fragPos;

uniform mat4 view;
uniform mat4 projection;

const float TEXTURE_LOOP_DIV = 2.0f;

void main() {
    uint skip = instanceFlags & 4;
    if (skip > 0) {
        gl_Position = vec4(0.0, 0.0, 0.0, 1.0);
        return;
    }

    gl_Position = projection * view * instanceMatrix * vec4(aPos, 1.0);
    vertexColor = aColor;
    uint extend_texture = instanceFlags & 1;
    fullbright = instanceFlags & 2;

    fragPos = vec3(instanceMatrix * vec4(aPos, 1.0));
    // move this to the cpu 
    normal = mat3(transpose(inverse(instanceMatrix))) * aNormal;

    // Texture coordinate
    if (extend_texture > 0) { // Loop texture
        // this is wrong fix it later
        // vec3 scaledNormal = normalize((instanceMatrix * vec4(normal, 0.0)).xyz);
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