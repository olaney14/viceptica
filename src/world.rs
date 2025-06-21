use cgmath::{Matrix4, SquareMatrix};

use crate::render::{self, Scene};

pub struct World {
    models: Vec<Model>,
    pub scene: render::Scene
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
    Mesh(String, Matrix4<f32>)
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