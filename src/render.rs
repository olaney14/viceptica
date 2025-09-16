use core::f32;
use std::{collections::HashMap, sync::LazyLock};

use cgmath::{point3, vec2, vec3, Deg, ElementWise, EuclideanSpace, InnerSpace, Matrix, Matrix3, Matrix4, Point3, SquareMatrix, Transform, Vector3, Zero};
use glow::{HasContext, NativeBuffer, NativeVertexArray};
use serde::{Deserialize, Serialize};
use winit::{event::MouseButton, keyboard::{Key, NamedKey}};

use crate::{collision::PhysicalProperties, common::{self, normal_matrix}, effects, input::Input, mesh::{self, flags, Mesh, MeshBank}, shader::{self, Program, ProgramBank}, texture::{Texture, TextureBank}, ui, world::{self, Model, Renderable, World}};

const HIDDEN_MASK_SIZE: f32 = 0.5;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RenderData {
    pub flags: u32,
    pub transform: Matrix4<f32>,
    pub normal_matrix: Matrix3<f32>
}

static DUMMY_RENDER_DATA_INSTANCED: LazyLock<RenderData> = LazyLock::new(|| {
    RenderData {
        flags: flags::SKIP,
        transform: Matrix4::identity(),
        normal_matrix: Matrix3::identity()
    }
});

#[derive(Clone, Copy, Debug)]
pub struct MobileRenderData {
    pub flags: u32,
    pub transform: Matrix4<f32>,
    pub normal_matrix: Matrix3<f32>,
    pub draw: bool,
    pub show_hidden: bool
}

static DUMMY_RENDER_DATA: LazyLock<MobileRenderData> = LazyLock::new(|| {
    MobileRenderData {
        flags: 0,
        transform: Matrix4::identity(),
        normal_matrix: Matrix3::identity(),
        draw: false,
        show_hidden: false
    }
});

#[derive(Clone, Copy, Debug)]
pub struct BillboardRenderData {
    pub flags: u32,
    pub position: Vector3<f32>,
    pub draw: bool,
    pub follow_vertical: bool,
    pub size: (f32, f32),
    pub show_hidden: bool
}

static DUMMY_BILLBOARD_DATA: LazyLock<BillboardRenderData> = LazyLock::new(|| {
    BillboardRenderData { 
        draw: false,
        flags: 0,
        follow_vertical: false,
        position: Vector3::zero(),
        size: (1.0, 1.0),
        show_hidden: false
    }
});

#[derive(Debug)]
pub struct Material {
    pub diffuse: String,
    pub specular: String,
    pub shininess: f32,
    pub physical_properties: PhysicalProperties
}

impl Material {
    pub fn new(diffuse: &str, specular: &str, shininess: f32) -> Self {
        Self {
            diffuse: diffuse.to_string(), shininess, specular: specular.to_string(), physical_properties: PhysicalProperties::default()
        }
    }

    pub fn with_physical_properties(diffuse: &str, specular: &str, shininess: f32, physical_properties: PhysicalProperties) -> Self {
        Self {
            diffuse: diffuse.to_string(), shininess, specular: specular.to_string(), physical_properties
        }
    }

    pub fn diffuse_only(diffuse: &str, shininess: f32) -> Self {
        Self::new(diffuse, "evil_pixel", shininess)
    }
}

#[derive(Clone)]
pub struct DirLight {
    pub direction: Vector3<f32>,
    pub ambient: Vector3<f32>,
    pub diffuse: Vector3<f32>,
    pub specular: Vector3<f32>
}

const FIXED_C: f32 = 1.0;
const FIXED_L: f32 = 0.0;

pub fn attenuation_coefficients(radius: f32, attenuation_threshold: f32) -> (f32, f32, f32) {
    if radius <= 0.01 {
        return (FIXED_C, FIXED_L, 100.0);
    }

    let inv_thresh = 1.0 / attenuation_threshold;
    let total = inv_thresh - FIXED_C;

    let quadratic = total / (radius * radius);

    (FIXED_C, FIXED_L, quadratic)
}

#[derive(Clone)]
pub struct PointLight {
    pub position: Vector3<f32>,
    pub constant: f32,
    pub linear: f32,
    pub quadratic: f32,
    pub ambient: Vector3<f32>,
    pub diffuse: Vector3<f32>,
    pub specular: Vector3<f32>,
    pub user_color: Option<Vector3<f32>>,
    pub user_attenuation: Option<f32>
}

impl PointLight {
    pub fn user_color_or_default(&self) -> Vector3<f32> {
        self.user_color.unwrap_or(self.diffuse)
    }

    pub fn user_attenuation_or_default(&self) -> f32 {
        self.user_attenuation.unwrap_or(10.0)
    }

    pub fn set_attenuation(&mut self, radius: f32) {
        let (constant, linear, quadratic) = attenuation_coefficients(radius, 0.05);
        self.user_attenuation = Some(radius);
        // println!("{}, {}, {}", constant, linear, quadratic);

        self.constant = constant;
        self.linear = linear;
        self.quadratic = quadratic;
    }

    pub fn set_color(&mut self, color: Vector3<f32>) {
        self.ambient = color * ui::implement::USER_AMBIENT_STRENGTH;
        self.diffuse = color;
        self.specular = common::vec3_mix(color, vec3(1.0, 1.0, 1.0), ui::implement::USER_SPECULAR_BLEND) * ui::implement::USER_SPECULAR_STRENGTH;
        self.user_color = Some(color);
    }

    pub fn default(position: Vector3<f32>) -> Self {
        let (constant, linear, quadratic) = attenuation_coefficients(10.0, 0.05);
        
        Self {
            ambient: vec3(0.1, 0.1, 0.1),
            diffuse: vec3(0.5, 0.5, 0.5),
            specular: vec3(1.0, 1.0, 1.0),
            constant, linear, quadratic,
            position,
            user_attenuation: None,
            user_color: None
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub enum Skybox {
    SolidColor(f32, f32, f32),
    Cubemap(String),
    NoClear
}

pub struct Environment {
    pub skybox: Skybox,
    pub dir_light: DirLight
}

impl Environment {
    pub fn new() -> Self {
        Self {
            skybox: Skybox::Cubemap(String::from("heaven")),
            dir_light: DirLight {
                direction: vec3(-0.2, -1.0, -0.3),
                ambient: vec3(0.3, 0.3, 0.3),
                diffuse: vec3(0.6, 0.6, 0.6),
                specular: vec3(0.75, 0.75, 0.75)
            }
        }
    }
}

pub struct Scene {
    /// Instance data for meshes that are changed infrequently<br>
    /// Data in here is written to individual buffers in `static_instance_buffers` during `prepare_statics` if it is marked as changed
    pub static_meshes: HashMap<String, Vec<RenderData>>,
    /// Used in `prepare_statics` to determine what static data needs to be rebuffered
    static_meshes_updated: Vec<String>,
    /// Instance buffers for each static model type, used in rendering and written to in `prepare_statics`
    static_instance_buffers: HashMap<String, NativeBuffer>,

    /// Meshed rendered individually
    pub mobile_meshes: HashMap<String, Vec<MobileRenderData>>,
    pub foreground_meshes: HashMap<String, Vec<MobileRenderData>>,
    pub billboards: HashMap<String, Vec<BillboardRenderData>>,
    pub camera: Camera,
    pub materials: HashMap<String, Material>,
    pub environment: Environment,
    pub point_lights: Vec<PointLight>,

    /// If true, `prepare_statics` will be called on the next frame
    pub statics_dirty: bool,

    pub skybox_vao: Option<NativeVertexArray>,
    pub window_size: (u32, u32),
    pub ui_vao: Option<NativeVertexArray>,
    pub show_hidden_objects: bool,
    pub applicable_materials: Vec<String>,
    pub post_process: effects::PostProcessing,
    pub world_default_effects: effects::DefaultEffects
}

impl Scene {
    /// load shaders, primitive meshes, materials
    pub unsafe fn init(&mut self, textures: &mut TextureBank, meshes: &mut MeshBank, programs: &mut ProgramBank, gl: &glow::Context) {
        programs.load_by_name_vf("instanced", gl).unwrap();
        programs.load_by_name_vf("flat", gl).unwrap();
        programs.load_by_name_vf("lines", gl).unwrap();
        programs.load_by_name_vf("skybox", gl).unwrap();
        programs.load_by_name_vf("screen", gl).unwrap();
        self.add_default_materials();
        self.applicable_materials = world::load_brushes(textures, meshes, self, gl);
        // billboards
        meshes.add(Mesh::create_square(1.0, 1.0, 1.0, gl), "quad");
        // textures.load_cubemap_by_name("field", gl).unwrap();
        // textures.load_cubemap_by_name("google", gl).unwrap();
        textures.load_cubemap_by_name("heaven", gl).unwrap();
        textures.load_by_name("stencil_hidden", gl).unwrap();
        self.skybox_vao = Some(mesh::create_skybox(gl));
        //textures.load_cubemap_by_name("heaven", gl).unwrap();
        //textures.load_cubemap_by_name("cloudy_sky", gl).unwrap();

        gl.enable(glow::DEPTH_TEST);
        gl.enable(glow::CULL_FACE);
    }

    pub unsafe fn update(&mut self, meshes: &mut MeshBank, gl: &glow::Context) {
        if self.statics_dirty {
            self.prepare_statics(meshes, gl);
            self.statics_dirty = false;
        }
    }

    unsafe fn stencil_hidden(&self, ui_program: &mut Program, textures: &TextureBank, gl: &glow::Context) {
        let hidden_stencil = textures.get("stencil_hidden").unwrap();
        gl.disable(glow::DEPTH_TEST);
        gl.disable(glow::CULL_FACE);
        gl.color_mask(false, false, false, false);
        gl.stencil_func(glow::ALWAYS, 1, 0xFF);
        gl.stencil_mask(0xFF);
        gl.depth_mask(false);
        gl.stencil_op(glow::REPLACE, glow::REPLACE, glow::REPLACE);

        // render hazard lines to stencil buffer
        gl.active_texture(glow::TEXTURE0);
        gl.bind_texture(glow::TEXTURE_2D, Some(hidden_stencil.inner));
        gl.bind_vertex_array(self.ui_vao);
        ui_program.uniform_1i32("tex", 0, gl);
        let size_y = (self.window_size.0 as f32 / self.window_size.1 as f32) * HIDDEN_MASK_SIZE;
        ui_program.uniform_2f32("texSize", vec2(HIDDEN_MASK_SIZE, size_y), gl);
        ui_program.uniform_2f32("pos", vec2(0.0, 0.0), gl);
        ui_program.uniform_2f32("scale", vec2(self.window_size.0 as f32, self.window_size.1 as f32), gl);
        ui_program.uniform_2f32("screenSize", vec2(self.window_size.0 as f32, self.window_size.1 as f32), gl);
        ui_program.uniform_2f32("texturePos", vec2(0.0, 0.0), gl);
        ui_program.uniform_2f32("textureScale", vec2(16.0, 16.0), gl);
        gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
    
        gl.stencil_op(glow::KEEP, glow::KEEP, glow::KEEP);
        gl.color_mask(true, true, true, true);
        gl.stencil_func(glow::EQUAL, 1, 0xFF);
        gl.depth_mask(true);
        gl.enable(glow::DEPTH_TEST);
        gl.enable(glow::CULL_FACE);
    }

    unsafe fn render_single_billboard(&self, data: &BillboardRenderData, quad: &Mesh, program: &mut Program, texture: &str, textures: &TextureBank, gl: &glow::Context) {
        let forward = if data.follow_vertical {
            (self.camera.pos.to_vec() - data.position).normalize()
        } else {
            let mut f = -self.camera.direction;
            f.y = 0.0;
            f.normalize()
        }; 

        let right = self.camera.up.cross(forward).normalize();
        let up = forward.cross(right);

        let view_rot = Matrix3::from_cols(right, up, forward);

        let transform = Matrix4::from_translation(data.position) * Matrix4::from_nonuniform_scale(data.size.0, data.size.1, 1.0) * common::mat3_to_mat4(view_rot);
        program.uniform_matrix4f32("model", transform, gl);
        program.uniform_1i32("flags", data.flags as i32, gl);
        program.uniform_1f32("material.shininess", 1.0, gl);
        gl.active_texture(glow::TEXTURE0);
        gl.bind_texture(glow::TEXTURE_2D, textures.get(texture).map(|s| s.inner));
        gl.active_texture(glow::TEXTURE1);
        gl.bind_texture(glow::TEXTURE_2D, textures.get("evil_pixel").map(|s| s.inner));
        gl.bind_vertex_array(Some(quad.vao));

        gl.draw_elements(
            glow::TRIANGLES,
            quad.indices as i32,
            glow::UNSIGNED_SHORT,
            0
        );
    }

    /// Call while flat program is being used
    unsafe fn render_billboards(&self, meshes: &MeshBank, program: &mut Program, textures: &TextureBank, gl: &glow::Context) {
        let mesh = meshes.get("quad").expect("no quad mesh");
        
        for (texture, data) in self.billboards.iter() {
            for data in data.iter() {
                if !data.draw { continue; }
                
                self.render_single_billboard(data, mesh, program, texture, textures, gl);
            }
        }
    }

    unsafe fn render_hidden_billboards(&self, meshes: &MeshBank, program: &mut Program, textures: &TextureBank, gl: &glow::Context) {
        let mesh = meshes.get("quad").expect("no quad mesh");

        for (texture, data) in self.billboards.iter() {
            for data in data {
                if !data.draw && data.show_hidden {
                    self.render_single_billboard(data, mesh, program, texture, textures, gl);
                }
            }
        }
    }

    pub unsafe fn render(&self, meshes: &MeshBank, programs: &mut ProgramBank, textures: &TextureBank, gl: &glow::Context) {
        // Clear screen
        match &self.environment.skybox {
            Skybox::SolidColor(r, g, b) => {
                gl.clear_color(*r, *g, *b, 1.0);
                gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);
            },
            Skybox::Cubemap(_) => {
                gl.clear_color(0.0, 0.0, 0.0, 1.0);
                gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);
            },
            Skybox::NoClear => {
                gl.clear(glow::DEPTH_BUFFER_BIT);
            }
        }

        // Render instanced
        let instanced_program = programs.get_mut("instanced").unwrap();
        gl.use_program(Some(instanced_program.inner));

        // Camera uniforms
        instanced_program.uniform_matrix4f32("view", self.camera.view, gl);
        instanced_program.uniform_matrix4f32("projection", self.camera.projection, gl);
        instanced_program.uniform_3f32("viewPos", self.camera.pos.to_vec(), gl);

        // Material uniforms
        instanced_program.uniform_1i32("material.diffuse", 0, gl);
        instanced_program.uniform_1i32("material.specular", 1, gl);

        // Lights
        self.uniform_lights(instanced_program, gl);

        // For each current static model type
        for (name, _) in self.static_instance_buffers.iter() {
            let mesh = meshes.get(name).unwrap();
            let material = self.materials.get(&mesh.material).unwrap();

            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, textures.get(&material.diffuse).map(|s| s.inner));
            gl.active_texture(glow::TEXTURE1);
            gl.bind_texture(glow::TEXTURE_2D, textures.get(&material.specular).map(|f| f.inner));
            gl.bind_vertex_array(Some(mesh.vao_instanced));
            
            instanced_program.uniform_1f32("material.shininess", material.shininess, gl);

            gl.draw_elements_instanced(
                glow::TRIANGLES,
                mesh.indices as i32,
                glow::UNSIGNED_SHORT,
                0,
                self.static_meshes.get(name).unwrap().len() as i32
            );
        }

        // Render individual
        let flat_program = programs.get_mut("flat").unwrap();
        gl.use_program(Some(flat_program.inner));

        // Camera
        flat_program.uniform_matrix4f32("view", self.camera.view, gl);
        flat_program.uniform_matrix4f32("projection", self.camera.projection, gl);
        flat_program.uniform_3f32("viewPos", self.camera.pos.to_vec(), gl);

        // Material
        flat_program.uniform_1i32("material.diffuse", 0, gl);
        flat_program.uniform_1i32("material.specular", 1, gl);

        // Lights
        self.uniform_lights(flat_program, gl);
        
        // For all types of mobile meshes
        for (name, data) in self.mobile_meshes.iter() {
            self.render_individual(data, name, meshes, textures, flat_program, gl);
        }

        self.render_billboards(meshes, flat_program, textures, gl);

        if self.show_hidden_objects {
            gl.clear_stencil(0);
            gl.clear(glow::STENCIL_BUFFER_BIT);
            gl.enable(glow::STENCIL_TEST);
            let ui_program = programs.get_mut("ui").unwrap();
            gl.use_program(Some(ui_program.inner));
            self.stencil_hidden(ui_program, textures, gl);

            let flat_program = programs.get_mut("flat").unwrap();
            gl.use_program(Some(flat_program.inner));

            for (name, data) in self.mobile_meshes.iter() {
                self.render_hidden(data, name, meshes, textures, flat_program, gl);
            }

            self.render_hidden_billboards(meshes, flat_program, textures, gl);

            gl.disable(glow::STENCIL_TEST);
        }

        // Render cubemap skybox
        if let Skybox::Cubemap(cubemap) = &self.environment.skybox {
            // https://learnopengl.com/Advanced-OpenGL/Cubemaps
            gl.depth_func(glow::LEQUAL);
            let skybox_program = programs.get_mut("skybox").unwrap();
            let cubemap_texture = textures.get_cubemap(cubemap).unwrap();
            gl.use_program(Some(skybox_program.inner));

            let modified_view = common::mat4_remove_translation(self.camera.view);
            skybox_program.uniform_matrix4f32("projection", self.camera.projection, gl);
            skybox_program.uniform_matrix4f32("view", modified_view, gl);

            gl.bind_vertex_array(self.skybox_vao);
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_CUBE_MAP, Some(cubemap_texture.inner));
            gl.draw_arrays(glow::TRIANGLES, 0, 36);

            gl.depth_func(glow::LESS);
        }

        // this has to be duplicated because of borrowing rules :(((((

        // Render individual
        let flat_program = programs.get_mut("flat").unwrap();
        gl.use_program(Some(flat_program.inner));

        gl.disable(glow::DEPTH_TEST);
        // For all types of foreground meshes
        for (name, data) in self.foreground_meshes.iter() {
            self.render_individual(data, name, meshes, textures, flat_program, gl);
        }
        gl.enable(glow::DEPTH_TEST);
    }

    pub unsafe fn debug_render_box(&self, transform: Matrix4<f32>, color: Vector3<f32>, box_vao: NativeVertexArray, programs: &mut ProgramBank, gl: &glow::Context) {
        gl.disable(glow::DEPTH_TEST);
        gl.line_width(2.0);

        let lines_program = programs.get_mut("lines").unwrap();
        gl.use_program(Some(lines_program.inner));
        gl.bind_vertex_array(Some(box_vao));

        lines_program.uniform_3f32("color", color, gl);
        lines_program.uniform_matrix4f32("view", self.camera.view, gl);
        lines_program.uniform_matrix4f32("projection", self.camera.projection, gl);
        lines_program.uniform_matrix4f32("model", transform, gl);

        gl.draw_elements(glow::LINES, 24, glow::UNSIGNED_SHORT, 0);
        gl.bind_vertex_array(None);

        gl.enable(glow::DEPTH_TEST);
    }


    #[inline]
    unsafe fn render_single_mesh(&self, data: &MobileRenderData, textures: &TextureBank, program: &mut Program, material: &Material, mesh: &Mesh, gl: &glow::Context) {
        program.uniform_matrix4f32("model", data.transform, gl);
        program.uniform_matrix3f32("normal_matrix", data.normal_matrix, gl);
        program.uniform_1i32("flags", data.flags as i32, gl);
        program.uniform_1f32("material.shininess", material.shininess, gl);
        gl.active_texture(glow::TEXTURE0);
        gl.bind_texture(glow::TEXTURE_2D, textures.get(&material.diffuse).map(|s| s.inner));
        gl.active_texture(glow::TEXTURE1);
        gl.bind_texture(glow::TEXTURE_2D, textures.get(&material.specular).map(|s| s.inner));
        gl.bind_vertex_array(Some(mesh.vao));

        gl.draw_elements(
            glow::TRIANGLES,
            mesh.indices as i32,
            glow::UNSIGNED_SHORT,
            0
        );
    }

    #[inline]
    unsafe fn render_individual(&self, data: &[MobileRenderData], name: &String, meshes: &MeshBank, textures: &TextureBank, program: &mut shader::Program, gl: &glow::Context) {
        let mesh = meshes.get(name).unwrap_or_else(|| panic!("Missing mesh \"{}\"", name));
        let material = self.materials.get(&mesh.material).unwrap_or_else(|| panic!("Missing material \"{}\"", mesh.material));

        for data in data.iter() {
            // Skip drawing if this is set as invisible
            if !data.draw { continue; }

            // Set transform and flags individually instead as of part of the instance buffer
            self.render_single_mesh(data, textures, program, material, mesh, gl);
        }
    }

    #[inline]
    unsafe fn render_hidden(&self, data: &[MobileRenderData], name: &String, meshes: &MeshBank, textures: &TextureBank, program: &mut Program, gl: &glow::Context) {
        let mesh = meshes.get(name).unwrap_or_else(|| panic!("Missing mesh \"{}\"", name));
        let material = self.materials.get(&mesh.material).unwrap_or_else(|| panic!("Missing material \"{}\"", mesh.material));

        for data in data {
            if !data.draw && data.show_hidden {
                self.render_single_mesh(data, textures, program, material, mesh, gl);
            }
        }
    }

    #[inline]
    unsafe fn uniform_lights(&self, program: &mut shader::Program, gl: &glow::Context) {
        program.uniform_1i32("pointLightCount", self.point_lights.len().min(64) as i32, gl);

        for i in 0..(self.point_lights.len().min(64)) {
            let light = self.point_lights.get(i).unwrap();
            program.uniform_3f32(&format!("pointLights[{}].position", i), light.position, gl);
            program.uniform_1f32(&format!("pointLights[{}].constant", i), light.constant, gl);
            program.uniform_1f32(&format!("pointLights[{}].linear", i), light.linear, gl);
            program.uniform_1f32(&format!("pointLights[{}].quadratic", i), light.quadratic, gl);
            program.uniform_3f32(&format!("pointLights[{}].ambient", i), light.ambient, gl);
            program.uniform_3f32(&format!("pointLights[{}].diffuse", i), light.diffuse, gl);
            program.uniform_3f32(&format!("pointLights[{}].specular", i), light.specular, gl);
        }

        program.uniform_3f32("dirLight.direction", self.environment.dir_light.direction, gl);
        program.uniform_3f32("dirLight.ambient", self.environment.dir_light.ambient, gl);
        program.uniform_3f32("dirLight.diffuse", self.environment.dir_light.diffuse, gl);
        program.uniform_3f32("dirLight.specular", self.environment.dir_light.specular, gl);
    }

    /// Add a static mesh to the render scene
    fn add_static_mesh(&mut self, mesh: &str, transform: Matrix4<f32>, flags: u32) {
        if let Some(transforms) = self.static_meshes.get_mut(mesh) {
            transforms.push(RenderData { transform, flags, normal_matrix: normal_matrix(transform) });
        } else {
            self.static_meshes.insert(mesh.to_string(), vec![RenderData { transform, flags, normal_matrix: normal_matrix(transform) }]);
        }
    }

    /// Add a mobile mesh to the render scene
    fn add_mobile_mesh(&mut self, mesh: &str, transform: Matrix4<f32>, flags: u32) {
        if let Some(transforms) = self.mobile_meshes.get_mut(mesh) {
            transforms.push(MobileRenderData { transform, flags, draw: true, normal_matrix: normal_matrix(transform), show_hidden: false });
        } else {
            self.mobile_meshes.insert(mesh.to_string(), vec![MobileRenderData { transform, flags, draw: true, normal_matrix: normal_matrix(transform), show_hidden: false }]);
        }
    }

    /// Add a foreground mesh to the render scene (no depth test, drawn last)
    fn add_foreground_mesh(&mut self, mesh: &str, transform: Matrix4<f32>, flags: u32) {
        if let Some(transforms) = self.foreground_meshes.get_mut(mesh) {
            transforms.push(MobileRenderData { transform, flags, draw: true, normal_matrix: normal_matrix(transform), show_hidden: false });
        } else {
            self.foreground_meshes.insert(mesh.to_string(), vec![MobileRenderData { transform, flags, draw: true, normal_matrix: normal_matrix(transform), show_hidden: false }]);
        }
    }

    fn add_billboard(&mut self, texture: &str, position: Vector3<f32>, size: (f32, f32), flags: u32, follow_vertical: bool) {
        if let Some(data) = self.billboards.get_mut(texture) {
            data.push(BillboardRenderData { position, flags, size, follow_vertical, draw: true, show_hidden: false });
        } else {
            self.billboards.insert(texture.to_string(), vec![BillboardRenderData { position, flags, size, follow_vertical, draw: true, show_hidden: false }]);
        }
    }

    fn insert_mesh_from_model(&mut self, name: &String, transform: &Matrix4<f32>, flags: u32, model: &Model, renderable_indices: &mut Vec<usize>) {
        if model.foreground {
            self.add_foreground_mesh(name, model.transform * transform, flags);
            renderable_indices.push(self.foreground_meshes.get(name).unwrap().len() - 1);
        } else if model.mobile {
            self.add_mobile_mesh(name, model.transform * transform, flags);
            renderable_indices.push(self.mobile_meshes.get(name).unwrap().len() - 1);
        } else {
            self.add_static_mesh(name, model.transform * transform, flags);
            if !self.static_meshes_updated.contains(name) {
                self.static_meshes_updated.push(name.to_string());
            }
            renderable_indices.push(self.static_meshes.get(name).unwrap().len() - 1);
            self.statics_dirty = true;
        }
    }

    /// Insert a model into the world and render scene
    pub fn insert_model(&mut self, model: &Model) -> Vec<usize> {
        let mut renderable_indices = Vec::new();
        for renderable in model.render.iter() {
            match renderable {
                Renderable::Mesh(name, transform, flags) => {
                    self.insert_mesh_from_model(name, transform, *flags, model, &mut renderable_indices);
                },
                Renderable::Brush(texture, position, size, flags) => {
                    let name = format!("Brush_{}", texture);
                    let transform = Matrix4::from_translation(*position) * Matrix4::from_nonuniform_scale(size.x, size.y, size.z);
                    self.insert_mesh_from_model(&name, &transform, *flags, model, &mut renderable_indices);
                },
                Renderable::Billboard(texture, position, size, flags, follow_vertical) => {
                    let transformed_position = model.transform.transform_point(Point3::from_vec(*position)).to_vec();
                    self.add_billboard(texture.as_str(), transformed_position, *size, *flags, *follow_vertical);
                    renderable_indices.push(self.billboards.get(texture).unwrap().len() - 1);
                }
            }
        }

        renderable_indices
    }

    /// Insert a new renderable into a preexisting model
    pub fn amend_model(&mut self, model: &mut Model, renderable: Renderable) {
        match renderable {
            Renderable::Mesh(ref name, transform, flags) => {
                let mut renderable_indices = Vec::new();
                self.insert_mesh_from_model(name, &transform, flags, model, &mut renderable_indices);
                model.renderable_indices.append(&mut renderable_indices);
            },
            Renderable::Brush(ref material, position, size, flags) => {
                let name = format!("Brush_{}", material);
                let transform = Matrix4::from_translation(position) * Matrix4::from_nonuniform_scale(size.x, size.y, size.z);
                let mut renderable_indices = Vec::new();
                self.insert_mesh_from_model(&name, &transform, flags, model, &mut renderable_indices);
                model.renderable_indices.append(&mut renderable_indices);
            },
            Renderable::Billboard(ref texture, position, size, flags, follow_vertical) => {
                self.add_billboard(texture.as_str(), position, size, flags, follow_vertical);
                model.renderable_indices.push(self.billboards.get(texture).unwrap().len() - 1);
            }
        }
        
        model.render.push(renderable);
    }

    fn remove_mesh(&mut self, data_index: usize, name: &String, model: &Model) {
        if model.foreground {
            self.foreground_meshes.get_mut(name).unwrap()[data_index] = *DUMMY_RENDER_DATA;
        } else if model.mobile {
            self.mobile_meshes.get_mut(name).unwrap()[data_index] = *DUMMY_RENDER_DATA;
        } else {
            self.static_meshes.get_mut(name).unwrap()[data_index] = *DUMMY_RENDER_DATA_INSTANCED;
            self.mark_static(name);
        }
    }

    /// "Removes" a renderable (replaces it with dummy data for the time being **TODO** btw)<br>
    /// Make sure to update collider references
    pub fn remove_renderable(&mut self, model: &mut Model, index: usize) {
        let data_index = model.renderable_indices[index];
        match model.render.get(index).as_ref().unwrap() {
            Renderable::Brush(material, _, _, _) => {
                let name = format!("Brush_{}", material);
                self.remove_mesh(data_index, &name, model);
            },
            Renderable::Mesh(name, _, _) => {
                self.remove_mesh(data_index, name, model);
            },
            Renderable::Billboard(texture, _, _, _, _) => {
                *self.billboards.get_mut(texture).unwrap().get_mut(index).unwrap() = *DUMMY_BILLBOARD_DATA;
            }
        }

        model.render.remove(index);
        model.renderable_indices.remove(index);
    }

    pub unsafe fn load_texture_to_material(&mut self, texture: &str, textures: &mut TextureBank, gl: &glow::Context) {
        textures.load_by_name(texture, gl).unwrap();
        self.add_material(Material::new(texture, "evil_pixel", 32.0), texture);
    }

    pub unsafe fn load_material_diff_spec(&mut self, name: &str, diffuse: &str, specular: &str, textures: &mut TextureBank, gl: &glow::Context) {
        textures.load_by_name(diffuse, gl).unwrap();
        textures.load_by_name(specular, gl).unwrap();
        self.add_material(Material::new(diffuse, specular, 32.0), name);
    }

    pub unsafe fn load_material_diff_spec_phys(&mut self, name: &str, diffuse: &str, specular: &str, phys: PhysicalProperties, textures: &mut TextureBank, gl: &glow::Context) {
        textures.load_by_name(diffuse, gl).unwrap();
        textures.load_by_name(specular, gl).unwrap();
        self.add_material(Material::with_physical_properties(diffuse, specular, 32.0, phys), name);
    }

    /// Mark a static mesh group for rebuffering
    pub fn mark_static(&mut self, name: &String) {
        if !self.static_meshes_updated.contains(name) {
            self.static_meshes_updated.push(name.clone());
            self.statics_dirty = true;
        }
    }

    fn update_model_transform_common(&mut self, renderable: &Renderable, index: usize, model_transform: Matrix4<f32>) {
        match renderable {
            Renderable::Billboard(texture, position, _, _, _) => {
                self.billboards.get_mut(texture).unwrap()[index].position = model_transform.transform_point(Point3::from_vec(*position)).to_vec();
            },
            _ => unreachable!()
        }
    }

    /// Just updates the transform for a mobile mesh,<br>
    /// But when updating a static mesh all other instances of the same type must be rebuffered so be careful
    pub fn update_model_transform(&mut self, model: &Model) {
        for (renderable, index) in model.render.iter().zip(model.renderable_indices.iter()) {
            if renderable.render_as_mesh() {
                let (mesh_transform, name) = match renderable {
                    Renderable::Mesh(name, transform, _) => (model.transform * transform, name),
                    Renderable::Brush(texture, position, size, _) => (
                        model.transform * Matrix4::from_translation(*position) * Matrix4::from_nonuniform_scale(size.x, size.y, size.z),
                        &format!("Brush_{}", texture)
                    ),
                    _ => unreachable!()
                };
                if model.mobile || model.foreground {
                    let meshes = if model.foreground {
                        self.foreground_meshes.get_mut(name).unwrap()
                    } else {
                        self.mobile_meshes.get_mut(name).unwrap()
                    };
                    meshes[*index].transform = mesh_transform;
                    meshes[*index].normal_matrix = normal_matrix(mesh_transform);
                } else {
                    let meshes = self.static_meshes.get_mut(name).unwrap();
                    meshes[*index].transform = mesh_transform;
                    meshes[*index].normal_matrix = normal_matrix(mesh_transform);
                    self.mark_static(name);
                }
            } else {
                self.update_model_transform_common(renderable, *index, model.transform);
            }
        }
    }

    pub fn new(gl: &glow::Context) -> Self {
        Self {
            mobile_meshes: HashMap::new(),
            static_instance_buffers: HashMap::new(),
            static_meshes: HashMap::new(),
            foreground_meshes: HashMap::new(),
            static_meshes_updated: Vec::new(),
            camera: Camera::new(),
            materials: HashMap::new(),
            environment: Environment::new(),
            point_lights: Vec::new(),
            statics_dirty: false,
            skybox_vao: None,
            billboards: HashMap::new(),
            window_size: (640 * 2, 480 * 2),
            ui_vao: None,
            show_hidden_objects: false,
            applicable_materials: Vec::new(),
            post_process: unsafe { effects::PostProcessing::new(gl) },
            world_default_effects: effects::DefaultEffects::new()
        }
    }

    pub fn add_point_light(&mut self, light: PointLight) -> usize {
        self.point_lights.push(light);

        if self.point_lights.len() > 64 {
            eprintln!("Warning: Too many point lights in scene");
        }
        
        self.point_lights.len() - 1
    }

    /// Rebuffers all changed static models<br>
    /// Clears `static_meshes_updated`
    pub unsafe fn prepare_statics(&mut self, meshes: &mut MeshBank, gl: &glow::Context) {
        for updated in self.static_meshes_updated.drain(..) {
            let new_buffer = if let Some(buffer) = self.static_instance_buffers.get_mut(&updated) {
                gl.delete_buffer(*buffer);
                *buffer = gl.create_buffer().unwrap();
                buffer
            } else {
                let buffer = gl.create_buffer().unwrap();
                self.static_instance_buffers.insert(updated.to_string(), buffer);
                self.static_instance_buffers.get(&updated).unwrap()
            };

            let render_data = self.static_meshes.get(&updated).unwrap();

            let instance_data: &[u8] = core::slice::from_raw_parts(
                render_data.as_ptr() as *const u8,
                render_data.len() * core::mem::size_of::<RenderData>()
            );
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(*new_buffer));
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, instance_data, glow::STATIC_DRAW);
        
            let mesh = meshes.meshes.get_mut(&updated).expect("Failed to get mesh");
            gl.bind_vertex_array(Some(mesh.vao_instanced));
            Mesh::define_instanced_vertex_attributes(gl);
            gl.bind_vertex_array(None);
        }
    }

    pub fn add_material(&mut self, material: Material, name: &str) {
        self.materials.insert(name.to_string(), material);
    }
}

#[derive(Clone)]
pub enum CameraControlScheme {
    FirstPerson(bool),
    Editor
}

pub struct Camera {
    pub pos: Point3<f32>,
    pub direction: Vector3<f32>,
    pub up: Vector3<f32>,
    pub right: Vector3<f32>,
    pub view: Matrix4<f32>,
    pub inverse_view: Matrix4<f32>,
    pub projection: Matrix4<f32>,
    pub inverse_projection: Matrix4<f32>,
    pub speed: f32,
    pub control_sceme: CameraControlScheme,
    pub pitch: f32,
    pub yaw: f32,
    pub sensitivity: f32,
    fov: f32,
    aspect: f32
}

impl Camera {
    pub fn new() -> Self {
        let mut camera = Self {
            pos: point3(0.0, 0.0, 3.0),
            direction: vec3(0.0, 0.0, -1.0),
            right: vec3(1.0, 0.0, 0.0),
            up: vec3(0.0, 1.0, 0.0),
            view: Matrix4::identity(),
            inverse_view: Matrix4::identity(),
            projection: cgmath::perspective(Deg(80.0), 640.0 / 480.0, 0.1, 100.0),
            inverse_projection: Matrix4::identity(),
            speed: 3.5,
            control_sceme: CameraControlScheme::FirstPerson(false), 
            pitch: 0.0,
            yaw: -f32::consts::PI / 2.0,
            sensitivity: 0.007,
            fov: 80.0,
            aspect: 640.0 / 480.0
        };
        camera.inverse_projection = camera.projection.invert().unwrap();
        camera
    }

    pub fn on_window_resized(&mut self, width: f32, height: f32) {
        self.projection = cgmath::perspective(Deg(self.fov), width / height, 0.1, 100.0);
        self.inverse_projection = self.projection.invert().unwrap();
    }

    pub fn set_fov(&mut self, new_fov: f32) {
        self.fov = new_fov;
        self.projection = cgmath::perspective(Deg(self.fov), self.aspect, 0.1, 100.0);
        self.inverse_projection = self.projection.invert().unwrap();
    }

    fn calculate_direction(&mut self) {
        self.direction.x = self.yaw.cos() * self.pitch.cos();
        self.direction.y = self.pitch.sin();
        self.direction.z = self.yaw.sin() * self.pitch.cos();
        self.direction = self.direction.normalize();
    }

    pub fn mouse_movement(&mut self, dx: f64, dy: f64, input: &Input) {
        match self.control_sceme {
            CameraControlScheme::Editor => {
                if input.get_mouse_button_pressed(MouseButton::Right) {
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
            CameraControlScheme::FirstPerson(locked) => {
                if locked {
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
        }
    }

    pub fn update(&mut self, input: &Input, delta_time: f32) {
        match self.control_sceme {
            CameraControlScheme::Editor => {
                if !input.get_key_pressed(Key::Named(NamedKey::Control)) {
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
                }
            },
            // Camera is moved by the player in this state
            CameraControlScheme::FirstPerson(_) => ()
        }

        self.right = vec3(0.0, 1.0, 0.0).cross(self.direction).normalize();
        self.up = self.direction.cross(self.right);

        self.view = Matrix4::look_at_rh(self.pos, self.pos + self.direction, vec3(0.0, 1.0, 0.0));
        self.inverse_view = self.view.invert().unwrap();
    }
}

impl Scene {
    pub fn add_default_materials(&mut self) {
        self.add_material(Material::new("magic_pixel", "evil_pixel", 32.0), "default");
    }
}

impl World {
    pub unsafe fn post_render(&self, programs: &mut ProgramBank, gl: &glow::Context) {
        if self.editor_data.active && self.editor_data.selection_box_visible {
            gl.disable(glow::DEPTH_TEST);
            gl.line_width(2.0);
            assert!(self.editor_data.selection_box_vao.is_some());
            gl.bind_vertex_array(self.editor_data.selection_box_vao);
            let lines_program = programs.get_mut("lines").unwrap();
            gl.use_program(Some(lines_program.inner));
            lines_program.uniform_3f32("color", vec3(0.0, 0.0, 1.0), gl);
            lines_program.uniform_matrix4f32("view", self.scene.camera.view, gl);
            lines_program.uniform_matrix4f32("projection", self.scene.camera.projection, gl);
            let model = 
                Matrix4::from_translation(self.editor_data.selection_box_pos) *
                Matrix4::from_nonuniform_scale(self.editor_data.selection_box_scale.x * 2.0, self.editor_data.selection_box_scale.y * 2.0, self.editor_data.selection_box_scale.z * 2.0);
            
            lines_program.uniform_matrix4f32("model", model, gl);
            gl.draw_elements(glow::LINES, 24, glow::UNSIGNED_SHORT, 0);
            gl.bind_vertex_array(None);
            gl.enable(glow::DEPTH_TEST);
        }
    }

    pub unsafe fn debug_render_colliders(&self, programs: &mut ProgramBank, gl: &glow::Context) {
        for collider in self.physical_scene.colliders.iter() {
            if let Some(collider) = collider {
                // skip player
                if collider.model.is_none() { continue; }
                let pos = vec3(collider.bounding.center().x, collider.bounding.center().y, collider.bounding.center().z);
                let scale = vec3(collider.bounding.half_extents().x, collider.bounding.half_extents().y, collider.bounding.half_extents().z);
                let model = 
                    Matrix4::from_translation(pos) *
                    Matrix4::from_nonuniform_scale(scale.x * 2.0, scale.y * 2.0, scale.z * 2.0);
                self.scene.debug_render_box(model, vec3(1.0, 0.0, 0.0), self.editor_data.selection_box_vao.unwrap(), programs, gl);
                match collider.shape {
                    crate::ColliderShape::Cuboid(cuboid) => {
                        let scale = vec3(cuboid.half_extents.x, cuboid.half_extents.y, cuboid.half_extents.z) * 2.0;
                        let tna = collider.iso.to_matrix();
                        let transform = Matrix4::new(
                            tna.m11, tna.m12, tna.m13, tna.m14,
                            tna.m21, tna.m22, tna.m23, tna.m24,
                            tna.m31, tna.m32, tna.m33, tna.m34,
                            tna.m41, tna.m42, tna.m43, tna.m44,
                        ).transpose() * Matrix4::from_nonuniform_scale(scale.x, scale.y, scale.z);
                        self.scene.debug_render_box(transform, vec3(0.4, 0.1, 0.8), self.editor_data.selection_box_vao.unwrap(), programs, gl);
                    }
                }
            }
        }
    }
}