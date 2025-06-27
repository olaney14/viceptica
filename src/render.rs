use core::f32;
use std::collections::HashMap;

use cgmath::{point3, vec3, Deg, ElementWise, EuclideanSpace, InnerSpace, Matrix4, Point3, Rad, SquareMatrix, Vector3};
use glow::{HasContext, NativeBuffer};
use winit::keyboard::Key;

use crate::{input::Input, mesh::{Mesh, MeshBank}, shader::ProgramBank, texture::TextureBank, world::{Model, Renderable}};

#[repr(C)]
#[derive(Clone, Copy)]
struct RenderData {
    flags: u32,
    transform: Matrix4<f32>
}

pub struct Scene {
    // eventually make this <String, all uniforms>
    pub static_meshes: HashMap<String, Vec<RenderData>>,
    static_meshes_updated: Vec<String>,
    static_instance_buffers: HashMap<String, NativeBuffer>,
    pub mobile_meshes: HashMap<String, Vec<Matrix4<f32>>>,
    pub camera: Camera,
}

impl Scene {
    /// load shaders, primitive meshes
    pub unsafe fn init(&mut self, programs: &mut ProgramBank, gl: &glow::Context) {
        programs.load_by_name_vf("instanced", gl).unwrap();
        programs.load_by_name_vf("flat", gl).unwrap();

        gl.enable(glow::DEPTH_TEST);
    }

    pub unsafe fn render(&self, meshes: &MeshBank, programs: &mut ProgramBank, textures: &TextureBank, gl: &glow::Context) {
        let instanced_program = programs.get_mut("instanced").unwrap();
        gl.use_program(Some(instanced_program.inner));
        instanced_program.uniform_1i32("textureIn", 0, gl);
        instanced_program.uniform_matrix4f32("view", self.camera.view, gl);
        instanced_program.uniform_matrix4f32("projection", self.camera.projection, gl);
        instanced_program.uniform_3f32("lightColor", vec3(1.0, 1.0, 1.0), gl);
        instanced_program.uniform_3f32("lightPos", vec3(0.0, -2.0, 0.0), gl);
        instanced_program.uniform_3f32("viewPos", self.camera.pos.to_vec(), gl);

        for (name, buffer) in self.static_instance_buffers.iter() {
            let mesh = meshes.get(name).unwrap();

            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, textures.get(&mesh.texture).map(|s| s.inner));
            gl.bind_vertex_array(Some(mesh.vao_instanced));

            gl.draw_elements_instanced(
                glow::TRIANGLES,
                mesh.indices as i32,
                glow::UNSIGNED_SHORT,
                0,
                self.static_meshes.get(name).unwrap().len() as i32
            );
        }

        let flat_program = programs.get_mut("flat").unwrap();
        gl.use_program(Some(flat_program.inner));
        flat_program.uniform_1i32("textureIn", 0, gl);
        flat_program.uniform_matrix4f32("view", self.camera.view, gl);
        flat_program.uniform_matrix4f32("projection", self.camera.projection, gl);
        
        for (name, transforms) in self.mobile_meshes.iter() {
            let mesh = meshes.get(name).unwrap();

            for transform in transforms.iter() {
                flat_program.uniform_matrix4f32("model", *transform, gl);
                gl.active_texture(glow::TEXTURE0);
                gl.bind_texture(glow::TEXTURE_2D, textures.get(&mesh.texture).map(|s| s.inner));
                gl.bind_vertex_array(Some(mesh.vao));

                gl.draw_elements(
                    glow::TRIANGLES,
                    mesh.indices as i32,
                    glow::UNSIGNED_SHORT,
                    0
                );
            }
        }
    }

    fn add_static_mesh(&mut self, mesh: &str, transform: Matrix4<f32>, flags: u32) {
        if let Some(transforms) = self.static_meshes.get_mut(mesh) {
            transforms.push(RenderData { transform, flags });
        } else {
            self.static_meshes.insert(mesh.to_string(), vec![RenderData { transform, flags }]);
        }
    }

    fn add_mobile_mesh(&mut self, mesh: &str, transform: Matrix4<f32>) {
        if let Some(transforms) = self.mobile_meshes.get_mut(mesh) {
            transforms.push(transform);
        } else {
            self.mobile_meshes.insert(mesh.to_string(), vec![transform]);
        }
    }

    pub fn insert_model(&mut self, model: &Model) -> Vec<usize> {
        let mut renderable_indices = Vec::new();
        for renderable in model.render.iter() {
            match renderable {
                Renderable::Mesh(name, transform, flags) => {
                    if model.mobile {
                        self.add_mobile_mesh(name, model.transform * transform);
                        renderable_indices.push(self.mobile_meshes.get(name).unwrap().len() - 1);
                    } else {
                        self.add_static_mesh(name, model.transform * transform, *flags);
                        if !self.static_meshes_updated.contains(name) {
                            self.static_meshes_updated.push(name.to_string());
                        }
                        renderable_indices.push(self.static_meshes.get(name).unwrap().len() - 1);
                    }
                },
                Renderable::Brush(texture, position, size, flags) => {
                    let name = format!("Brush_{}", texture);
                    let transform = Matrix4::from_translation(*position) * Matrix4::from_nonuniform_scale(size.x, size.y, size.z);
                    if model.mobile {
                        self.add_mobile_mesh(&name, model.transform * transform);
                        renderable_indices.push(self.mobile_meshes.get(&name).unwrap().len() - 1);
                    } else {
                        self.add_static_mesh(&name, transform, *flags);
                        if !self.static_meshes_updated.contains(&name) {
                            self.static_meshes_updated.push(name.clone());
                        }
                        renderable_indices.push(self.static_meshes.get(&name).unwrap().len() - 1);
                    }
                }
            }
        }

        renderable_indices
    }

    pub fn update_model_transform(&mut self, model: &Model) {
        if !model.mobile {
            unimplemented!();
        }

        for (renderable, index) in model.render.iter().zip(model.renderable_indices.iter()) {
            match renderable {
                Renderable::Mesh(name, transform, flags) => {
                    self.mobile_meshes.get_mut(name).unwrap()[*index] = model.transform * transform;
                },
                Renderable::Brush(texture, position, size, flags) => {
                    let name = format!("Brush_{}", texture);
                    let transform = Matrix4::from_translation(*position) * Matrix4::from_nonuniform_scale(size.x, size.y, size.z);
                    self.mobile_meshes.get_mut(&name).unwrap()[*index] = model.transform * transform;
                }
            }
        }
    }

    pub fn new() -> Self {
        Self {
            mobile_meshes: HashMap::new(),
            static_instance_buffers: HashMap::new(),
            static_meshes: HashMap::new(),
            static_meshes_updated: Vec::new(),
            camera: Camera::new()
        }
    }

    pub unsafe fn prepare_statics(&mut self, meshes: &mut MeshBank, gl: &glow::Context) {
        for updated in self.static_meshes_updated.iter() {
            let new_buffer = if let Some(buffer) = self.static_instance_buffers.get_mut(updated) {
                gl.delete_buffer(*buffer);
                *buffer = gl.create_buffer().unwrap();
                buffer
            } else {
                let buffer = gl.create_buffer().unwrap();
                self.static_instance_buffers.insert(updated.to_string(), buffer);
                self.static_instance_buffers.get(updated).unwrap()
            };

            let render_data = self.static_meshes.get(updated).unwrap();

            let instance_data: &[u8] = core::slice::from_raw_parts(
                render_data.as_ptr() as *const u8,
                render_data.len() * core::mem::size_of::<RenderData>()
            );
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(*new_buffer));
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, instance_data, glow::STATIC_DRAW);
        
            let mesh = meshes.meshes.get_mut(updated).unwrap();
            gl.bind_vertex_array(Some(mesh.vao_instanced));
            Mesh::define_instanced_vertex_attributes(gl);
            gl.bind_vertex_array(None);
        }
    }
}

pub struct Camera {
    pub pos: Point3<f32>,
    pub direction: Vector3<f32>,
    pub up: Vector3<f32>,
    pub right: Vector3<f32>,
    pub view: Matrix4<f32>,
    pub projection: Matrix4<f32>,
    pub speed: f32,
    pub mouse_locked: bool,
    pub pitch: f32,
    pub yaw: f32,
    pub sensitivity: f32,
    fov: f32,
    aspect: f32
}

impl Camera {
    pub fn new() -> Self {
        Self {
            pos: point3(0.0, 0.0, 3.0),
            direction: vec3(0.0, 0.0, -1.0),
            right: vec3(1.0, 0.0, 0.0),
            up: vec3(0.0, 1.0, 0.0),
            view: Matrix4::identity(),
            projection: cgmath::perspective(Deg(80.0), 640.0 / 480.0, 0.1, 100.0),
            speed: 3.5,
            mouse_locked: false,
            pitch: 0.0,
            yaw: -f32::consts::PI / 2.0,
            sensitivity: 0.007,
            fov: 80.0,
            aspect: 640.0 / 480.0
        }
    }

    pub fn on_window_resized(&mut self, width: f32, height: f32) {
        self.projection = cgmath::perspective(Deg(self.fov), width / height, 0.1, 100.0);
    }

    pub fn set_fov(&mut self, new_fov: f32) {
        self.fov = new_fov;
        self.projection = cgmath::perspective(Deg(self.fov), self.aspect, 0.1, 100.0);
    }

    fn calculate_direction(&mut self) {
        self.direction.x = self.yaw.cos() * self.pitch.cos();
        self.direction.y = self.pitch.sin();
        self.direction.z = self.yaw.sin() * self.pitch.cos();
        self.direction = self.direction.normalize();
    }

    pub fn mouse_movement(&mut self, dx: f64, dy: f64) {
        if self.mouse_locked {
            self.yaw += dx as f32 * self.sensitivity;
            self.pitch += dy as f32 * self.sensitivity;

            if self.pitch > (f32::consts::PI / 2.0) - 0.025 {
                self.pitch = (f32::consts::PI / 2.0) - 0.025;
            } else if self.pitch < (-f32::consts::PI / 2.0) + 0.025 {
                self.pitch = (-f32::consts::PI / 2.0) + 0.025;
            }

            self.calculate_direction();
        }
    }

    pub fn update(&mut self, input: &Input, delta_time: f32) {
        if input.get_key_pressed(Key::Character("w".into())) {
            self.pos += self.speed * delta_time * self.direction.normalize();
        }
        if input.get_key_pressed(Key::Character("s".into())) {
            self.pos -= self.speed * delta_time * self.direction.normalize();
        }
        if input.get_key_pressed(Key::Character("a".into())) {
            self.pos += self.speed * delta_time * self.up.cross(self.direction).normalize().mul_element_wise(vec3(1.0, 0.0, 1.0));
        }
        if input.get_key_pressed(Key::Character("d".into())) {
            self.pos -= self.speed * delta_time * self.up.cross(self.direction).normalize().mul_element_wise(vec3(1.0, 0.0, 1.0));
        }
        if input.get_key_pressed(Key::Character("e".into())) {
            self.pos += self.speed * delta_time * self.up.normalize();
        }
        if input.get_key_pressed(Key::Character("q".into())) {
            self.pos -= self.speed * delta_time * self.up.normalize();
        }

        self.right = vec3(0.0, 1.0, 0.0).cross(self.direction).normalize();
        self.up = self.direction.cross(self.right);

        self.view = Matrix4::look_at_rh(self.pos, self.pos + self.direction, vec3(0.0, 1.0, 0.0));
    }
}