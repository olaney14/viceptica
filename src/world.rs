use cgmath::{Matrix4, SquareMatrix, Vector3};

use crate::{mesh::{Mesh, MeshBank}, render::{self, Scene}, texture::TextureBank};

pub const BRUSH_TEXTURES: [&str; 7] = [
    "concrete",
    "end_sky",
    "evilwatering",
    "pillows_old_floor",
    "sky",
    "sparkle",
    "watering"
];

pub struct World {
    models: Vec<Model>,
    pub scene: render::Scene
}

pub unsafe fn load_brushes(textures: &mut TextureBank, meshes: &mut MeshBank, gl: &glow::Context) {
    for texture in BRUSH_TEXTURES.iter() {
        textures.load_by_name(&texture, gl).unwrap();
        meshes.add(Mesh::create_textured_cube(&texture, gl), &format!("Brush_{}", texture));
    }
}

impl World {
    pub fn new() -> Self {
        Self {
            models: Vec::new(),
            scene: Scene::new()
        }
    }

    pub fn insert_model(&mut self, mut model: Model) {
        model.renderable_indices = self.scene.insert_model(&model);
        self.models.push(model);
    }

    pub fn set_model_transform(&mut self, index: usize, new_transform: Matrix4<f32>) {
        let model = self.models.get_mut(index).unwrap();
        if !model.mobile {
            unimplemented!();
        }

        model.transform = new_transform;
        self.scene.update_model_transform(model);
    }
}

#[derive(Clone)]
pub enum Renderable {
    Mesh(String, Matrix4<f32>, u32),
    Brush(String, Vector3<f32>, Vector3<f32>, u32)
}

#[derive(Clone)]
pub struct Model {
    pub transform: Matrix4<f32>,
    pub render: Vec<Renderable>,
    pub mobile: bool,
    pub renderable_indices: Vec<usize>
}

impl Model {
    pub fn new() -> Self {
        Self {
            transform: Matrix4::identity(),
            render: Vec::new(),
            mobile: false,
            renderable_indices: Vec::new()
        }
    }
}