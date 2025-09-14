use cgmath::{vec3, Vector3};
use glow::{HasContext, NativeFramebuffer, NativeTexture, NativeVertexArray};

use crate::shader::{Program, ProgramBank};

pub struct KernelEffect {
    pub kernel: [f32; 9],
    pub offset: f32
}

pub struct FogEffect {
    pub color: Vector3<f32>,
    pub strength: f32,
    pub max: f32
}

pub struct PostProcessing {
    pub fbo: NativeFramebuffer,
    pub texture_color: Option<NativeTexture>,
    pub texture_depth: Option<NativeTexture>,
    pub error: Vec<String>,
    pub dummy_vao: NativeVertexArray,
    pub kernel: Option<KernelEffect>,
    pub fog: Option<FogEffect>
}

impl PostProcessing {
    pub unsafe fn new(gl: &glow::Context) -> Self {
        let fbo = gl.create_framebuffer().unwrap();
        let vao = gl.create_vertex_array().unwrap();

        Self {
            fbo,
            texture_color: None,
            texture_depth: None,
            error: Vec::new(),
            dummy_vao: vao,
            fog: None,
            kernel: None
        }
    }

    pub unsafe fn resize(&mut self, window_size: (u32, u32), gl: &glow::Context) {
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.fbo));

        let color_attachment = gl.create_texture().unwrap();
        gl.bind_texture(glow::TEXTURE_2D, Some(color_attachment));
        gl.tex_image_2d(
            glow::TEXTURE_2D, 0, glow::RGB as i32, 
            window_size.0 as i32, window_size.1 as i32, 
            0, glow::RGB, glow::UNSIGNED_BYTE,
            glow::PixelUnpackData::Slice(None)
        );
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::LINEAR as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::LINEAR as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE as i32);
        
        gl.framebuffer_texture_2d(
            glow::FRAMEBUFFER, glow::COLOR_ATTACHMENT0,
            glow::TEXTURE_2D, Some(color_attachment), 0
        );
        self.texture_color = Some(color_attachment);

        let depth_attachment = gl.create_texture().unwrap();
        gl.bind_texture(glow::TEXTURE_2D, Some(depth_attachment));
        gl.tex_image_2d(
            glow::TEXTURE_2D, 0, glow::DEPTH24_STENCIL8 as i32,
            window_size.0 as i32, window_size.1 as i32,
            0, glow::DEPTH_STENCIL, glow::UNSIGNED_INT_24_8,
            glow::PixelUnpackData::Slice(None)
        );
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::LINEAR as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::LINEAR as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE as i32);
        
        gl.framebuffer_texture_2d(
            glow::FRAMEBUFFER, glow::DEPTH_STENCIL_ATTACHMENT,
            glow::TEXTURE_2D, Some(depth_attachment), 0
        );

        if gl.check_framebuffer_status(glow::FRAMEBUFFER) != glow::FRAMEBUFFER_COMPLETE {
            self.error.push(String::from("Error: framebuffer was not complete"));
        }
        gl.bind_framebuffer(glow::FRAMEBUFFER, None);
        self.texture_depth = Some(depth_attachment);
    }

    pub unsafe fn begin(&self, gl: &glow::Context) {
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.fbo));
    }

    pub unsafe fn end(&self, programs: &mut ProgramBank, gl: &glow::Context) {
        gl.bind_framebuffer(glow::FRAMEBUFFER, None);
        gl.clear_color(1.0, 1.0, 1.0, 1.0);
        gl.clear_depth(1.0);
        gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT | glow::STENCIL_BUFFER_BIT);

        let screen_program = programs.get_mut("screen").unwrap();
        gl.use_program(Some(screen_program.inner));
        screen_program.uniform_1i32("screenTexture", 0, gl);
        gl.active_texture(glow::TEXTURE0);
        gl.bind_texture(glow::TEXTURE_2D, Some(self.texture_color.unwrap()));
    
        screen_program.uniform_1i32("depthTexture", 1, gl);
        gl.active_texture(glow::TEXTURE1);
        gl.bind_texture(glow::TEXTURE_2D, Some(self.texture_depth.unwrap()));

        self.uniform_fog(screen_program, gl);
        self.uniform_kernel(screen_program, gl);

        gl.bind_vertex_array(Some(self.dummy_vao));
        gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
    }

    unsafe fn uniform_kernel(&self, program: &mut Program, gl: &glow::Context) {
        if let Some(kernel) = &self.kernel {
            program.uniform_1i32("kernel.flags", 1, gl);
            program.uniform_3f32("kernel.top",    vec3(kernel.kernel[0], kernel.kernel[1], kernel.kernel[2]), gl);
            program.uniform_3f32("kernel.middle", vec3(kernel.kernel[3], kernel.kernel[4], kernel.kernel[5]), gl);
            program.uniform_3f32("kernel.bottom", vec3(kernel.kernel[6], kernel.kernel[7], kernel.kernel[8]), gl);
            program.uniform_1f32("kernel.offset", kernel.offset, gl);
        } else {
            program.uniform_1i32("kernel.flags", 0, gl);
        }
    }

    unsafe fn uniform_fog(&self, program: &mut Program, gl: &glow::Context) {
        if let Some(fog) = &self.fog {
            program.uniform_1i32("fog.flags", 1, gl);
            program.uniform_3f32("fog.color", fog.color, gl);
            program.uniform_1f32("fog.strength", fog.strength, gl);
            program.uniform_1f32("fog.max", fog.max, gl);
        } else {
            program.uniform_1i32("fog.flags", 0, gl);
        }
    }
}