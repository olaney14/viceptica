out vec4 FragColor;

in vec2 TexCoord;

struct Fog {
    int flags;

    vec3 color;
    float strength;
    float max;
};
uniform Fog fog;

struct Kernel {
    int flags;

    vec3 top;
    vec3 middle;
    vec3 bottom;
    float offset;
};
uniform Kernel kernel;

uniform sampler2D screenTexture;
uniform sampler2D depthTexture;

void main() {
    vec3 color = vec3(texture(screenTexture, TexCoord));
    int kernel_enabled = kernel.flags & 1;
    int fog_enabled = fog.flags & 1;

    if (kernel_enabled == 1) {
        // https://learnopengl.com/Advanced-OpenGL/Framebuffers
        vec2 offsets[9] = vec2[](
            vec2(-kernel.offset,  kernel.offset), // top-left
            vec2( 0.0f,    kernel.offset), // top-center
            vec2( kernel.offset,  kernel.offset), // top-right
            vec2(-kernel.offset,  0.0f),   // center-left
            vec2( 0.0f,    0.0f),   // center-center
            vec2( kernel.offset,  0.0f),   // center-right
            vec2(-kernel.offset, -kernel.offset), // bottom-left
            vec2( 0.0f,   -kernel.offset), // bottom-center
            vec2( kernel.offset, -kernel.offset)  // bottom-right    
        );

        float user_kernel[9] = float[](
            kernel.top.x, kernel.top.y, kernel.top.z,
            kernel.middle.x, kernel.middle.y, kernel.middle.z,
            kernel.bottom.x, kernel.bottom.y, kernel.bottom.z
        );

        vec3 sampleTex[9];
        for (int i = 0; i < 9; i++) {
            sampleTex[i] = vec3(texture(screenTexture, TexCoord.st + offsets[i]));
        }

        color = vec3(0.0);
        for (int i = 0; i < 9; i++)
            color += sampleTex[i] * user_kernel[i];
    }

    if (fog_enabled == 1) {
        float fog_strength = min(fog.max, pow(texture(depthTexture, TexCoord).r, fog.strength));
        color = mix(color, fog.color, fog_strength);
    }

    FragColor = vec4(color, 1.0);
}