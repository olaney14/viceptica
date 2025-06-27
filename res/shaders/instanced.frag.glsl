out vec4 FragColor;

in vec3 vertexColor;
in vec2 TexCoord;
flat in uint fullbright;
in vec3 normal;
in vec3 fragPos;

uniform sampler2D textureIn;
uniform vec3 lightColor;
uniform vec3 lightPos;
uniform vec3 viewPos;

void main() {
    FragColor = texture(textureIn, TexCoord) * vec4(vertexColor, 1.0f);

    if (fullbright == 0) {
        // ambient
        float ambientStrength = 0.1;
        vec3 ambient = ambientStrength * lightColor;

        // diffuse
        vec3 norm = normalize(normal);
        vec3 lightDir = normalize(lightPos - fragPos);
        float diff = max(dot(norm, lightDir), 0.0);
        vec3 diffuse = diff * lightColor;

        float specularStrength = 0.5;
        vec3 viewDir = normalize(viewPos - fragPos);
        vec3 reflectDir = reflect(-lightDir, norm);
        float spec = pow(max(dot(viewDir, reflectDir), 0.0), 32);
        vec3 specular = specularStrength * spec * lightColor;

        vec3 result = (ambient + diffuse + specular) * FragColor.xyz;
        FragColor = vec4(result, 1.0);
    }
}