use std::{collections::HashMap, error::Error, fs, io::Read, os, path::PathBuf};

use cgmath::Matrix4;
use glow::{HasContext, NativeUniformLocation};

const SHADER_VERSION: &str = "#version 410";

pub struct Program {
    pub name: String,
    pub inner: glow::Program,
    pub uniform_locations: HashMap<String, Option<NativeUniformLocation>>
}

// https://github.com/grovesNL/glow/blob/main/examples/hello/src/main.rs
impl Program {
    pub unsafe fn from_vert_frag(vert: &str, frag: &str, name: &str, gl: &glow::Context) -> Self {
        let shader_sources = [
            (glow::VERTEX_SHADER, vert),
            (glow::FRAGMENT_SHADER, frag)
        ];

        let program = gl.create_program().expect("Cannot create program");
        let mut shaders = Vec::with_capacity(2);

        for (shader_type, source) in shader_sources.iter() {
            let shader = gl
                .create_shader(*shader_type)
                .expect("Cannot create shader");

            gl.shader_source(shader, &format!("{}\n{}", SHADER_VERSION, source));
            gl.compile_shader(shader);
            if !gl.get_shader_compile_status(shader) {
                panic!("{}", gl.get_shader_info_log(shader));
            }
            gl.attach_shader(program, shader);
            shaders.push(shader);
        }

        gl.link_program(program);
        if !gl.get_program_link_status(program) {
            panic!("{}", gl.get_program_info_log(program));
        }

        for shader in shaders {
            gl.detach_shader(program, shader);
            gl.delete_shader(shader);
        }
        
        Self {
            name: name.to_string(),
            inner: program,
            uniform_locations: HashMap::new()
        }
    }

    unsafe fn get_uniform_location(&mut self, loc: &str, gl: &glow::Context) -> Option<&NativeUniformLocation> {
        if !self.uniform_locations.contains_key(loc) {
            self.uniform_locations.insert(
                loc.to_string(),
                gl.get_uniform_location(self.inner, loc)
            );
        }

        self.uniform_locations.get(loc).unwrap().as_ref()
    }

    pub unsafe fn uniform1i32(&mut self, loc: &str, value: i32, gl: &glow::Context) {
        gl.uniform_1_i32(self.get_uniform_location(loc, gl), value);
    }

    pub unsafe fn uniform_matrix4f32(&mut self, loc: &str, value: Matrix4<f32>, gl: &glow::Context) {
        let matrix_as_slice: [[f32; 4]; 4] = value.into();
        gl.uniform_matrix_4_f32_slice(self.get_uniform_location(loc, gl), false, &matrix_as_slice.as_flattened());
    }
}

pub struct ProgramBank {
    pub programs: HashMap<String, Program>
}

impl ProgramBank {
    pub fn new() -> Self {
        Self {
            programs: HashMap::new()
        }
    }

    pub fn add(&mut self, name: &str, program: Program) {
        self.programs.insert(name.to_string(), program);
    }

    pub fn get_inner(&self, name: &str) -> glow::Program {
        self.programs[name].inner
    }

    pub fn get(&self, name: &str) -> Option<&Program> {
        self.programs.get(name)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut Program> {
        self.programs.get_mut(name)
    }

    pub unsafe fn load_by_name_vf(&mut self, name: &str, gl: &glow::Context) -> Result<(), Box<dyn Error>> {
        if self.programs.contains_key(name) {
            eprintln!("Program was already loaded");
            return Ok(());
        }

        let mut vertex_file = fs::File::open(PathBuf::from(format!("res/shaders/{}.vert.glsl", name)))?;
        let mut vertex_src = String::new();
        vertex_file.read_to_string(&mut vertex_src)?;

        let mut frag_file = fs::File::open(PathBuf::from(format!("res/shaders/{}.frag.glsl", name)))?;
        let mut frag_src = String::new();
        frag_file.read_to_string(&mut frag_src)?;

        Ok(self.add(name, Program::from_vert_frag(&vertex_src, &frag_src, name, gl)))
    }
}