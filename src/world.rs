use core::f32;
use std::path::PathBuf;

use cgmath::{vec3, vec4, AbsDiffEq, ElementWise, EuclideanSpace, InnerSpace, Matrix4, Point3, Quaternion, Rad, Rotation, SquareMatrix, Vector3, Zero};
use glow::{HasContext, NativeVertexArray};
use parry3d::bounding_volume::BoundingVolume;
use winit::{event::MouseButton, keyboard::{Key, NamedKey}};

use crate::{collision::{Collider, PhysicalProperties, PhysicalScene, RaycastResult}, common, component::Component, input::Input, mesh::{flags, Mesh, MeshBank}, render::{self, Camera, Scene}, save::LevelData, shader::ProgramBank, texture::TextureBank};

pub const BRUSH_TEXTURES: [&str; 8] = [
    "concrete",
    "end_sky",
    "evilwatering",
    "pillows_old_floor",
    "sky",
    "sparkle",
    "watering",
    "container"
];

pub const APPLICABLE_MATERIALS: [&str; 10] = [
    "concrete",
    "end_sky",
    "evilwatering",
    "pillows_old_floor",
    "sky",
    "sparkle",
    "watering",
    "container",
    "ice",
    "tar"
];

pub const DEFAULT_INCREMENT: f32 = 0.25;

const ARROW_LOWEST_Y: f32 = -1.435;
const ARROW_HEIGHT: f32 = 3.471;
const EPSILON: f32 = 0.005;
const COYOTE: u32 = 3;

pub enum Selection {
    Brush(usize),
    Model(usize),
    Multiple(Vec<Selection>)
}

#[derive(Clone, Copy)]
pub enum SelectionType {
    Movement,
    Scaling,
    // Rotation
}

#[derive(Clone, Copy)]
pub enum DragAxis {
    X, Y, Z
}

impl SelectionType {
    pub fn cycle(&self) -> Self {
        match self {
            Self::Movement => Self::Scaling,
            Self::Scaling => Self::Movement,
            // _ => Self::Movement
        }
    }
}

pub struct EditorModeData {
    pub active: bool,
    pub selected_object: Option<Selection>,
    pub selection_type: SelectionType,
    pub drag_axis: Option<DragAxis>,
    pub init_drag_along_plane: Option<Vector3<f32>>,
    pub drag_distance: Option<f32>,
    pub drag_object_origin: Option<Vector3<f32>>,
    pub drag_object_scale: Option<Vector3<f32>>,
    pub drag_object_sign: Option<bool>,
    pub drag_plane: Option<Vector3<f32>>,
    pub increment: f32,
    pub selection_box_pos: Vector3<f32>,
    pub selection_box_scale: Vector3<f32>,
    pub selection_box_vao: Option<NativeVertexArray>,
    pub selection_box_visible: bool,
    pub apply_material: Option<String>,
    pub light_selected: Option<usize>,
    pub open_light_ui: Option<usize>,
    pub save_to: Option<PathBuf>,
    pub show_debug: Vec<String>,
    pub multiple_selection_offsets: Vec<Vector3<f32>>
}

impl EditorModeData {
    pub fn get_selected_brush(&self) -> Option<usize> {
        if let Some(Selection::Brush(brush)) = &self.selected_object {
            return Some(*brush);
        }

        None
    }

    pub fn get_selected_model(&self) -> Option<usize> {
        if let Some(Selection::Model(model)) = &self.selected_object {
            return Some(*model);
        }

        None
    }
}

pub struct World {
    pub models: Vec<Option<Model>>,
    pub scene: render::Scene,
    pub player: Player,
    pub physical_scene: PhysicalScene,
    pub gravity: f32,
    pub air_friction: f32,
    pub internal: InternalModels,
    pub editor_data: EditorModeData,
    pub load_new: Option<LevelData>,
    /// this many frames will be ignored
    pub freeze: u32,
    pub do_game_logic: bool
}

#[derive(Default)]
pub struct InternalModels {
    pub arrow_px: usize,
    pub arrow_nx: usize,
    pub arrow_py: usize,
    pub arrow_ny: usize,
    pub arrow_pz: usize,
    pub arrow_nz: usize,
    pub brushes: usize,
    pub debug_arrow: usize,
    pub box_px: usize,
    pub box_nx: usize,
    pub box_py: usize,
    pub box_ny: usize,
    pub box_pz: usize,
    pub box_nz: usize,
    pub internal_ids: Vec<usize>
}

pub unsafe fn load_brushes(textures: &mut TextureBank, meshes: &mut MeshBank, scene: &mut Scene, gl: &glow::Context) {
    for texture in BRUSH_TEXTURES.iter() {
        scene.load_material_diff_spec(texture, texture, &format!("{}_specular", texture), textures, gl);
        meshes.add(Mesh::create_material_cube(texture, gl), &format!("Brush_{}", texture));
    }

    scene.load_material_diff_spec_phys("ice", "ice", "ice_specular", PhysicalProperties {
        friction: 0.99,
        control: 0.05,
        jump: 1.0
    }, textures, gl);
    meshes.add(Mesh::create_material_cube("ice", gl), "Brush_ice");
    scene.load_material_diff_spec_phys("tar", "tar", "tar_specular", PhysicalProperties {
        friction: 0.25,
        control: 0.03,
        jump: 0.1
    }, textures, gl);
    meshes.add(Mesh::create_material_cube("tar", gl), "Brush_tar");
}

impl World {
    pub fn new() -> Self {
        let mut world = Self {
            models: Vec::new(),
            scene: Scene::new(),
            player: Player::new(),
            physical_scene: PhysicalScene::new(),
            gravity: 15.0,
            air_friction: 0.995,
            internal: InternalModels::default(),
            editor_data: EditorModeData {
                selected_object: None,
                selection_type: SelectionType::Movement,
                active: false,
                drag_axis: None,
                init_drag_along_plane: None,
                drag_object_origin: None,
                drag_distance: None,
                increment: DEFAULT_INCREMENT,
                selection_box_pos: Vector3::zero(),
                selection_box_scale: vec3(1.0, 1.0, 1.0),
                selection_box_visible: false,
                selection_box_vao: None,
                drag_plane: None,
                drag_object_scale: None,
                drag_object_sign: None,
                apply_material: None,
                light_selected: None,
                open_light_ui: None,
                save_to: None,
                show_debug: Vec::new(),
                multiple_selection_offsets: Vec::new()
            },
            load_new: None,
            freeze: 0,
            do_game_logic: true
        };

        world.player.collider = world.physical_scene.add_collider(Collider::cuboid(Vector3::zero(), vec3(0.5, 2.0, 0.5), Vector3::zero()));

        world
    }

    pub fn init(&mut self, meshes: &mut MeshBank, gl: &glow::Context) {
        meshes.load_from_obj("arrow", gl);
        meshes.load_from_obj_vcolor("arrow", "arrowred", 1.0, 0.0, 0.0, gl);
        meshes.load_from_obj_vcolor("arrow", "arrowgreen", 0.0, 1.0, 0.0, gl);
        meshes.load_from_obj_vcolor("arrow", "arrowblue", 0.0, 0.0, 1.0, gl);
        unsafe { 
            meshes.add(Mesh::create_colored_cube(1.0, 0.0, 0.0, gl), "cubered");
            meshes.add(Mesh::create_colored_cube(0.0, 1.0, 0.0, gl), "cubegreen");
            meshes.add(Mesh::create_colored_cube(0.0, 0.0, 1.0, gl), "cubeblue");
        }

        // Collider is slightly larger than the arrow to make them less annoying to click
        self.internal.arrow_px = self.insert_model(Model::from_loaded_file("arrowred", meshes).unwrap().fullbright().foreground().collider_cuboid(vec3(0.15, 0.0, 0.0), vec3(1.75, 0.375, 0.375) / 2.0).non_solid());
        self.internal.arrow_py = self.insert_model(Model::from_loaded_file("arrowblue", meshes).unwrap().fullbright().foreground().collider_cuboid(vec3(0.0, 0.15, 0.0), vec3(0.375, 1.75, 0.375) / 2.0).non_solid());
        self.internal.arrow_pz = self.insert_model(Model::from_loaded_file("arrowgreen", meshes).unwrap().fullbright().foreground().collider_cuboid(vec3(0.0, 0.0, 0.15), vec3(0.375, 0.375, 1.75) / 2.0).non_solid());
        self.internal.arrow_nx = self.insert_model(Model::from_loaded_file("arrowred", meshes).unwrap().fullbright().foreground().collider_cuboid(vec3(-0.15, 0.0, 0.0), vec3(1.75, 0.375, 0.375) / 2.0).non_solid());
        self.internal.arrow_ny = self.insert_model(Model::from_loaded_file("arrowblue", meshes).unwrap().fullbright().foreground().collider_cuboid(vec3(0.0, -0.15, 0.0), vec3(0.375, 1.75, 0.375) / 2.0).non_solid());
        self.internal.arrow_nz = self.insert_model(Model::from_loaded_file("arrowgreen", meshes).unwrap().fullbright().foreground().collider_cuboid(vec3(0.0, 0.0, -0.15), vec3(0.375, 0.375, 1.75) / 2.0).non_solid());
        self.internal.brushes = self.insert_model(Model::new(false, Matrix4::identity(), Vec::new()));
        self.internal.debug_arrow = self.insert_model(Model::from_loaded_file("arrow", meshes).unwrap().fullbright().mobile());
        self.internal.box_px = self.insert_model(Model::new(true, Matrix4::identity(), vec![ Renderable::Mesh("cubered".to_string(), Matrix4::identity(), flags::FULLBRIGHT) ]).collider_cuboid(Vector3::zero(), vec3(0.3, 0.3, 0.3)).foreground().non_solid());
        self.internal.box_nx = self.insert_model(Model::new(true, Matrix4::identity(), vec![ Renderable::Mesh("cubered".to_string(), Matrix4::identity(), flags::FULLBRIGHT) ]).collider_cuboid(Vector3::zero(), vec3(0.3, 0.3, 0.3)).foreground().non_solid());
        self.internal.box_py = self.insert_model(Model::new(true, Matrix4::identity(), vec![ Renderable::Mesh("cubeblue".to_string(), Matrix4::identity(), flags::FULLBRIGHT) ]).collider_cuboid(Vector3::zero(), vec3(0.3, 0.3, 0.3)).foreground().non_solid());
        self.internal.box_ny = self.insert_model(Model::new(true, Matrix4::identity(), vec![ Renderable::Mesh("cubeblue".to_string(), Matrix4::identity(), flags::FULLBRIGHT) ]).collider_cuboid(Vector3::zero(), vec3(0.3, 0.3, 0.3)).foreground().non_solid());
        self.internal.box_pz = self.insert_model(Model::new(true, Matrix4::identity(), vec![ Renderable::Mesh("cubegreen".to_string(), Matrix4::identity(), flags::FULLBRIGHT) ]).collider_cuboid(Vector3::zero(), vec3(0.3, 0.3, 0.3)).foreground().non_solid());
        self.internal.box_nz = self.insert_model(Model::new(true, Matrix4::identity(), vec![ Renderable::Mesh("cubegreen".to_string(), Matrix4::identity(), flags::FULLBRIGHT) ]).collider_cuboid(Vector3::zero(), vec3(0.3, 0.3, 0.3)).foreground().non_solid());
    
        self.internal.internal_ids.extend(vec![
            self.internal.arrow_px, self.internal.arrow_py, self.internal.arrow_pz,
            self.internal.arrow_nx, self.internal.arrow_ny, self.internal.arrow_nz,
            self.internal.brushes, self.internal.debug_arrow,
            self.internal.box_px, self.internal.box_py, self.internal.box_pz,
            self.internal.box_nx, self.internal.box_ny, self.internal.box_nz,
        ]);
    }

    /// set up editor data for movement/scaling and handle switching
    pub fn select_brush(&mut self, brush: usize) {
        if let Some(current) = self.editor_data.get_selected_brush() {
            if current == brush {
                match self.editor_data.selection_type {
                    SelectionType::Movement => {
                        self.set_arrows_visible(false);
                        self.set_boxes_visible(true);
                        self.move_arrows_far();
                    },
                    SelectionType::Scaling => {
                        self.set_arrows_visible(true);
                        self.set_boxes_visible(false);
                        self.move_boxes_far();
                    },
                    // _ => ()
                } 
                
                self.editor_data.selection_type = self.editor_data.selection_type.cycle();
            } else {
                self.editor_data.selection_type = SelectionType::Movement;
                self.editor_data.selected_object = Some(Selection::Brush(brush));
                self.set_boxes_visible(false);
                self.move_boxes_far();
            }
        } else {
            self.editor_data.selection_type = SelectionType::Movement;
            self.editor_data.selected_object = Some(Selection::Brush(brush));
            self.set_boxes_visible(false);
            self.move_boxes_far();
        }
    }

    // deduplicate these two somehow?
    pub fn select_model(&mut self, model: usize) {
        if !self.models[model].as_ref().unwrap().lights.is_empty() {
            self.editor_data.light_selected = Some(self.models[model].as_ref().unwrap().lights[0].1);
            self.editor_data.open_light_ui = self.editor_data.light_selected;
        } else {
            self.editor_data.light_selected = None;
        }

        if let Some(current) = self.editor_data.get_selected_model() {
            if current != model {
                self.editor_data.selection_type = SelectionType::Movement;
                self.editor_data.selected_object = Some(Selection::Model(model));
                self.set_boxes_visible(false);
                self.move_boxes_far();
            }
        } else {
            self.editor_data.selection_type = SelectionType::Movement;
            self.editor_data.selected_object = Some(Selection::Model(model));
            self.set_boxes_visible(false);
            self.move_boxes_far();
        }
    }

    pub fn select_or_append_model(&mut self, model: usize) {
        if self.editor_data.selected_object.is_none() {
            self.select_model(model);
        } else {
            match self.editor_data.selected_object.as_mut().unwrap() {
                Selection::Model(_) | Selection::Brush(_) => {
                    self.editor_data.selected_object = Some(Selection::Multiple(vec![
                        self.editor_data.selected_object.take().unwrap(),
                        Selection::Model(model)
                    ]));
                },
                Selection::Multiple(selected) => {
                    selected.push(Selection::Model(model));
                }
            }
            self.editor_data.light_selected = None;
            self.set_arrows_visible(true);
        }
    }

    pub fn select_or_append_brush(&mut self, brush: usize) {
        if self.editor_data.selected_object.is_none() {
            self.select_brush(brush);
        } else {
            match self.editor_data.selected_object.as_mut().unwrap() {
                Selection::Model(_) | Selection::Brush(_) => {
                    self.editor_data.selected_object = Some(Selection::Multiple(vec![
                        self.editor_data.selected_object.take().unwrap(),
                        Selection::Brush(brush)
                    ]));
                },
                Selection::Multiple(selected) => {
                    selected.push(Selection::Brush(brush));
                }
            }
            self.editor_data.light_selected = None;
        }
    }

    fn can_be_selected(&self, model: usize) -> bool {
        !self.internal.internal_ids.contains(&model)
    }

    fn get_models_or_brushes_within_rect(&self, x0: i32, y0: i32, x1: i32, y1: i32, window_width: u32, window_height: u32, brushes: bool) -> Vec<usize> {
        let to_clip = self.scene.camera.projection * self.scene.camera.view;
        let mut models_in_box = Vec::new();
        let x0f =  2.0 * ((x0.min(x1) as f32 / window_width as f32) - 0.5);
        let y0f = -2.0 * ((y0.max(y1) as f32 / window_height as f32) - 0.5);
        let x1f =  2.0 * ((x0.max(x1) as f32 / window_width as f32) - 0.5);
        let y1f = -2.0 * ((y0.min(y1) as f32 / window_height as f32) - 0.5);

        if brushes {
            for (i, brush) in self.models[self.internal.brushes].as_ref().unwrap().render.iter().enumerate() {
                if let Renderable::Brush(_, pos, size, _) = brush {
                    let pos = vec4(pos.x, pos.y, pos.z, 1.0);
                    let clip = to_clip * pos;
                    let ndc = clip.truncate() / clip.w;
                    if ndc.x > x0f && ndc.x < x1f && ndc.y > y0f && ndc.y < y1f && ndc.z < 1.0 {
                        models_in_box.push(i);
                    }
                }
            }
        } else {
            for model in self.models.iter() {
                if let Some(model) = model {
                    if self.can_be_selected(model.index.unwrap()) {
                        let pos = common::translation(model.transform);
                        let pos = vec4(pos.x, pos.y, pos.z, 1.0);
                        let clip = to_clip * pos;
                        let ndc = clip.truncate() / clip.w;
                        if ndc.x > x0f && ndc.x < x1f && ndc.y > y0f && ndc.y < y1f && ndc.z < 1.0 {
                            models_in_box.push(model.index.unwrap());
                        }
                    }
                }
            }
        }

        models_in_box
    }

    pub fn get_models_within_rect(&self, x0: i32, y0: i32, x1: i32, y1: i32, window_width: u32, window_height: u32) -> Vec<usize> {
        self.get_models_or_brushes_within_rect(x0, y0, x1, y1, window_width, window_height, false)
    }

    pub fn get_brushes_within_rect(&self, x0: i32, y0: i32, x1: i32, y1: i32, window_width: u32, window_height: u32) -> Vec<usize> {
        self.get_models_or_brushes_within_rect(x0, y0, x1, y1, window_width, window_height, true)
    }

    pub fn model_pressed(&mut self, result: RaycastResult) {
        if self.editor_data.active {
            if let Some(model) = result.model {
                if model == self.internal.arrow_nx || model == self.internal.arrow_px || model == self.internal.box_nx || model == self.internal.box_px {
                    self.editor_data.drag_axis = Some(DragAxis::X);
                    if self.scene.camera.direction.dot(Vector3::unit_y()).abs() > self.scene.camera.direction.dot(Vector3::unit_z()).abs() {
                        self.editor_data.drag_plane = Some(Vector3::unit_y());
                    } else {
                        self.editor_data.drag_plane = Some(Vector3::unit_z());
                    }
                } else if model == self.internal.arrow_ny || model == self.internal.arrow_py || model == self.internal.box_ny || model == self.internal.box_py {
                    self.editor_data.drag_axis = Some(DragAxis::Y);
                    if self.scene.camera.direction.dot(Vector3::unit_x()).abs() > self.scene.camera.direction.dot(Vector3::unit_z()).abs() {
                        self.editor_data.drag_plane = Some(Vector3::unit_x());
                    } else {
                        self.editor_data.drag_plane = Some(Vector3::unit_z());
                    }
                } else if model == self.internal.arrow_nz || model == self.internal.arrow_pz || model == self.internal.box_nz || model == self.internal.box_pz {
                    self.editor_data.drag_axis = Some(DragAxis::Z);
                    if self.scene.camera.direction.dot(Vector3::unit_x()).abs() > self.scene.camera.direction.dot(Vector3::unit_y()).abs() {
                        self.editor_data.drag_plane = Some(Vector3::unit_x());
                    } else {
                        self.editor_data.drag_plane = Some(Vector3::unit_y());
                    }
                }

                if model == self.internal.arrow_px || model == self.internal.arrow_py || model == self.internal.arrow_pz || model == self.internal.box_px || model == self.internal.box_py || model == self.internal.box_pz {
                    self.editor_data.drag_object_sign = Some(true);
                } else if model == self.internal.arrow_nx || model == self.internal.arrow_ny || model == self.internal.arrow_nz || model == self.internal.box_nx || model == self.internal.box_ny || model == self.internal.box_nz {
                    self.editor_data.drag_object_sign = Some(false);
                }
            }
        }
    }

    pub fn model_released(&mut self, result: RaycastResult, shift_pressed: bool) {
        if self.editor_data.active {
            if let Some(model) = result.model {
                if let Some(renderable) = result.renderable {
                    if model == self.internal.brushes {
                        if shift_pressed {
                            self.select_or_append_brush(renderable);
                        } else {
                            self.select_brush(renderable);
                        }
                        self.set_arrows_visible(true);
                    }
                }

                if self.can_be_selected(model) {
                    if shift_pressed {
                        self.select_or_append_model(model);
                    } else {
                        self.select_model(model);
                    }
                    self.set_arrows_visible(true);
                }
            }
        }
    }

    pub fn air_clicked(&mut self) {
        self.deselect();
        self.editor_data.selected_object = None;
    }

    fn pre_insert_model(&mut self, model: &mut Model) {
        model.insert_colliders(self);
        model.renderable_indices = self.scene.insert_model(model);

        let mut extents: Option<parry3d::bounding_volume::Aabb> = None;

        for renderable in model.render.iter() {
            if let Some(aabb) = renderable.get_extents() {
                if let Some(extents) = &mut extents {
                    extents.merge(&aabb);
                } else {
                    extents = Some(aabb);
                }
            }
        }

        if model.extents.is_none() {
            model.extents = extents.map(|aabb| {
                let center = aabb.center();
                let half_extents = aabb.half_extents();
                (
                    vec3(center.x, center.y, center.z),
                    vec3(half_extents.x, half_extents.y, half_extents.z)
                )
            });
        }

        for i in 0..model.components.len() {
            Component::on_insert(i, model, self);
        }
    }

    pub fn insert_model(&mut self, mut model: Model) -> usize {
        for light in model.lights.iter() {
            let position = light.0 + (model.transform * vec4(0.0, 0.0, 0.0, 1.0)).xyz();
            self.scene.point_lights[light.1].position = position;
        }

        for i in 0..self.models.len() {
            if self.models[i].is_none() {
                model.index = Some(i);
                self.pre_insert_model(&mut model);
                self.models[i] = Some(model);
                return i;
            }
        }

        model.index = Some(self.models.len());
        self.pre_insert_model(&mut model);
        self.models.push(Some(model));
        self.models.len() - 1
    }

    pub fn remove_model(&mut self, index: usize) -> Result<(), String> {
        if self.models[index].is_some() {
            for i in 0..self.models[index].as_ref().unwrap().lights.len() {
                self.remove_point_light(self.models[index].as_ref().unwrap().lights[i].1);
            }
        }

        if let Some(mut model) = self.models[index].take() {
            for i in 0..model.renderable_indices.len() {
                self.scene.remove_renderable(&mut model, i);
            }
            for i in 0..model.colliders.len() {
                if model.colliders[i].is_some() {
                    self.physical_scene.remove_collider(model.colliders[i].unwrap()).unwrap();
                }
            }
            Ok(())
        } else {
            Err("Index out of bounds".to_string())
        }
    }

    /// This also removes the point light from the model
    pub fn remove_point_light(&mut self, light: usize) {    
        let mut removed = false;
        for model in self.models.iter_mut().flatten() {
            let mut light_index = None;
            for (_, index) in model.lights.iter_mut() {
                if *index > light {
                    *index -= 1;
                } else if *index == light {
                    light_index = Some(*index);
                }
            }

            if let Some(model_light) = light_index {
                model.lights.remove(model_light);
                removed = true;
            }
        }

        self.scene.point_lights.remove(light);

        if !removed {
            eprintln!("Removed light was not found in any model");
        }
    }

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

    /// Panics if the internal brush model has been deleted or if `brush` is an invalid index
    pub fn set_brush_origin(&mut self, brush_index: usize, new_origin: Vector3<f32>) {
        self.set_brush_origin_scale(brush_index, new_origin, None);
    }

    pub fn set_brush_origin_scale(&mut self, brush_index: usize, new_origin: Vector3<f32>, new_scale: Option<Vector3<f32>>) {
        let brush = self.models[self.internal.brushes].as_mut().unwrap().render.get_mut(brush_index).unwrap();

        if let Renderable::Brush(material, origin, scale, _) = brush {
            *origin = new_origin;
            *scale = new_scale.unwrap_or(*scale);
            let name = format!("Brush_{}", material);
            self.scene.mark_static(&name);
            // this counts on the transform of self.internal.brushes being identity
            let transform = Matrix4::from_translation(*origin) * Matrix4::from_nonuniform_scale(scale.x, scale.y, scale.z);
            self.scene.static_meshes.get_mut(&name).unwrap()[self.models[self.internal.brushes].as_ref().unwrap().renderable_indices[brush_index]].transform = transform;
            self.update_colliders(self.internal.brushes);
        } else {
            panic!("Non-brush in internal brush model");
        }
    }

    /// Returns the new brush index
    pub fn set_brush_material(&mut self, brush_index: usize, new_material: String) -> usize {
        if matches!(self.models[self.internal.brushes].as_ref().unwrap().render.get(brush_index).unwrap(), Renderable::Brush(..)) {
            let mut new_brush = self.models[self.internal.brushes].as_ref().unwrap().render.get(brush_index).unwrap().clone();
            // self.scene.remove_renderable(self.models[self.internal.brushes].as_mut().unwrap(), brush_index);
            if let Renderable::Brush(material, ..) = &mut new_brush {
                *material = new_material;
            }
            
            //self.scene.amend_model(self.models[self.internal.brushes].as_mut().unwrap(), new_brush);
            self.remove_brush(brush_index);
            self.insert_brush(new_brush)
        } else {
            panic!("Non-brush in internal brush model");
        }
    }

    pub fn debug_brushes(&self) {
        println!("{:?}", self.models[self.internal.brushes].as_ref().unwrap().render);
    }

    pub fn insert_brush(&mut self, brush: Renderable) -> usize {
        match brush {
            Renderable::Brush(ref material, position, size, _) => {
                let model = self.models.get_mut(self.internal.brushes).unwrap().as_mut().unwrap();
                let model_position: Vector3<f32> = (model.transform * vec4(0.0, 0.0, 0.0, 1.0)).xyz();
                let properties = self.scene.materials.get(material).unwrap().physical_properties;
                let mut collider = Collider::cuboid(position + model_position, size, Vector3::zero());
                collider.physical_properties = properties;
                collider.renderable = Some(model.render.len());
                collider.model = Some(self.internal.brushes);
                model.colliders.push(Some(self.physical_scene.add_collider(collider)));
                self.scene.amend_model(model, brush);
                model.render.len() - 1
            },
            _ => panic!("thats not a brush")
        }
    }

    // oh my god i love rust
    pub fn set_model_transform(&mut self, index: usize, new_transform: Matrix4<f32>) {
        {
            let model = self.models.get_mut(index).unwrap().as_mut().unwrap();

            model.transform = new_transform;
            self.scene.update_model_transform(model);
            self.update_colliders(index);
        }
        {
            let model = self.models.get(index).unwrap().as_ref().unwrap();
            for light in model.lights.iter() {
                let position = light.0 + (new_transform* vec4(0.0, 0.0, 0.0, 1.0)).xyz();
                self.scene.point_lights[light.1].position = position;
            }
        }
    }

    /// only use this during component update or any other time the model has been taken
    pub fn set_model_transform_external(&mut self, model: Model, new_transform: Matrix4<f32>) -> Model {
        let index = model.index.unwrap();
        assert!(self.models[index].is_none());
        self.models[index] = Some(model);
        self.set_model_transform(index, new_transform);
        self.models[index].take().unwrap()
    }

    pub fn get_model_transform(&self, index: usize) -> Option<Matrix4<f32>> {
        self.models[index].as_ref().map(|o| o.transform)
    }

    fn position_arrows(&mut self, position: Vector3<f32>, scale: Vector3<f32>) {
        self.set_model_transform(self.internal.arrow_px, Matrix4::from_translation(position + vec3(scale.x, 0.0, 0.0)) * Matrix4::from_axis_angle(Vector3::unit_z(), Rad(-f32::consts::PI / 2.0)) * Matrix4::from_scale(0.5));
        self.set_model_transform(self.internal.arrow_nx, Matrix4::from_translation(position - vec3(scale.x, 0.0, 0.0)) * Matrix4::from_axis_angle(Vector3::unit_z(), Rad(f32::consts::PI / 2.0)) * Matrix4::from_scale(0.5));
        self.set_model_transform(self.internal.arrow_py, Matrix4::from_translation(position + vec3(0.0, scale.y, 0.0)) * Matrix4::from_scale(0.5));
        self.set_model_transform(self.internal.arrow_ny, Matrix4::from_translation(position - vec3(0.0, scale.y, 0.0)) * Matrix4::from_axis_angle(Vector3::unit_x(), Rad(f32::consts::PI)) * Matrix4::from_scale(0.5));
        self.set_model_transform(self.internal.arrow_pz, Matrix4::from_translation(position + vec3(0.0, 0.0, scale.z)) * Matrix4::from_axis_angle(Vector3::unit_x(), Rad(f32::consts::PI / 2.0)) * Matrix4::from_scale(0.5));
        self.set_model_transform(self.internal.arrow_nz, Matrix4::from_translation(position - vec3(0.0, 0.0, scale.z)) * Matrix4::from_axis_angle(Vector3::unit_x(), Rad(-f32::consts::PI / 2.0)) * Matrix4::from_scale(0.5));
    }

    fn adorn_arrows_model(&mut self, model: usize) {
        let (mut position, half_extents) = self.models[model].as_ref().unwrap().extents.unwrap_or((vec3(0.0, 0.0, 0.0), vec3(0.5, 0.5, 0.5)));
        position += (self.models[model].as_ref().unwrap().transform * vec4(0.0, 0.0, 0.0, 1.0)).xyz();
        let scale = half_extents + vec3(1.0, 1.0, 1.0);

        self.position_arrows(position, scale);

        self.editor_data.selection_box_visible = true;
        self.editor_data.selection_box_pos = position;
        self.editor_data.selection_box_scale = half_extents;
    }

    fn adorn_arrows_brush(&mut self, brush: usize) {
        let (position, scale) = self.get_brush_adornment_transform(brush);

        self.position_arrows(position, scale);
    
        self.editor_data.selection_box_visible = true;
        self.editor_data.selection_box_pos = position;
        self.editor_data.selection_box_scale = scale - vec3(1.0, 1.0, 1.0);
    }

    fn adorn_arrows_multiple(&mut self, multiple: &Vec<Selection>) {
        let (position, half_extents) = common::compose_extents(
            multiple.iter().map(|a| match a {
                Selection::Brush(index) => { 
                    let extents = self.get_brush_adornment_transform(*index);
                    // extents.0 += if let Renderable::Brush(_, pos, _, _) = self.models[self.internal.brushes].as_ref().unwrap().render[*index] { pos } else { unreachable!() };
                    (extents.0, extents.1 - vec3(1.0, 1.0, 1.0)) 
                },
                Selection::Model(index) => {
                    let position = common::translation(self.models[*index].as_ref().unwrap().transform);
                    let mut extents = self.models[*index].as_ref().unwrap().extents.unwrap_or((Vector3::zero(), vec3(0.5, 0.5, 0.5)));
                    extents.0 += position;
                    extents
                },
                _ => panic!("multiple selection within multiple selection")
            })
        );
        let scale = half_extents + vec3(1.0, 1.0, 1.0);

        self.position_arrows(position, scale);

        self.editor_data.selection_box_visible = true;
        self.editor_data.selection_box_pos = position;
        self.editor_data.selection_box_scale = half_extents;

        self.editor_data.multiple_selection_offsets.clear();
        for selection in multiple {
            self.editor_data.multiple_selection_offsets.push(
                match selection {
                    Selection::Brush(index) => {
                        if let Renderable::Brush(_, pos, _, _) = self.models[self.internal.brushes].as_ref().unwrap().render[*index] { pos - position } else { unreachable!() }
                    },
                    Selection::Model(model) => {
                        common::translation(self.models[*model].as_ref().unwrap().transform) - position
                    },
                    Selection::Multiple(_) => unreachable!()
                }
            );
        }
    }

    fn adorn_boxes_brush(&mut self, brush: usize) {
        let (position, mut scale) = self.get_brush_adornment_transform(brush);
        scale -= vec3(0.8, 0.8, 0.8);

        self.set_model_transform(self.internal.box_px, Matrix4::from_translation(position + vec3(scale.x, 0.0, 0.0)) * Matrix4::from_scale(0.25));
        self.set_model_transform(self.internal.box_nx, Matrix4::from_translation(position - vec3(scale.x, 0.0, 0.0)) * Matrix4::from_scale(0.25));
        self.set_model_transform(self.internal.box_py, Matrix4::from_translation(position + vec3(0.0, scale.y, 0.0)) * Matrix4::from_scale(0.25));
        self.set_model_transform(self.internal.box_ny, Matrix4::from_translation(position - vec3(0.0, scale.y, 0.0)) * Matrix4::from_scale(0.25));
        self.set_model_transform(self.internal.box_pz, Matrix4::from_translation(position + vec3(0.0, 0.0, scale.z)) * Matrix4::from_scale(0.25));
        self.set_model_transform(self.internal.box_nz, Matrix4::from_translation(position - vec3(0.0, 0.0, scale.z)) * Matrix4::from_scale(0.25));

        self.editor_data.selection_box_visible = true;
        self.editor_data.selection_box_pos = position;
        self.editor_data.selection_box_scale = scale - vec3(0.2, 0.2, 0.2);
    }

    pub fn move_arrows_far(&mut self) {
        self.set_model_transform(self.internal.arrow_px, Matrix4::from_translation(vec3(0.0, -1000.0, 0.0)));
        self.set_model_transform(self.internal.arrow_nx, Matrix4::from_translation(vec3(0.0, -1000.0, 0.0)));
        self.set_model_transform(self.internal.arrow_py, Matrix4::from_translation(vec3(0.0, -1000.0, 0.0)));
        self.set_model_transform(self.internal.arrow_ny, Matrix4::from_translation(vec3(0.0, -1000.0, 0.0)));
        self.set_model_transform(self.internal.arrow_pz, Matrix4::from_translation(vec3(0.0, -1000.0, 0.0)));
        self.set_model_transform(self.internal.arrow_nz, Matrix4::from_translation(vec3(0.0, -1000.0, 0.0)));
    }

    pub fn move_boxes_far(&mut self) {
        self.set_model_transform(self.internal.box_px, Matrix4::from_translation(vec3(0.0, -1000.0, 0.0)));
        self.set_model_transform(self.internal.box_nx, Matrix4::from_translation(vec3(0.0, -1000.0, 0.0)));
        self.set_model_transform(self.internal.box_py, Matrix4::from_translation(vec3(0.0, -1000.0, 0.0)));
        self.set_model_transform(self.internal.box_ny, Matrix4::from_translation(vec3(0.0, -1000.0, 0.0)));
        self.set_model_transform(self.internal.box_pz, Matrix4::from_translation(vec3(0.0, -1000.0, 0.0)));
        self.set_model_transform(self.internal.box_nz, Matrix4::from_translation(vec3(0.0, -1000.0, 0.0)));
    }

    fn get_brush_adornment_transform(&self, brush_index: usize) -> (Vector3<f32>, Vector3<f32>) {
        let brushes = self.models.get(self.internal.brushes).unwrap().as_ref().unwrap();
        let brush = brushes.render.get(brush_index).unwrap();
        if let Renderable::Brush(_, position, scale, _) = brush {
            return (*position, (*scale / 2.0) + vec3(1.0, 1.0, 1.0))
        }
        unreachable!();
    }

    /// Provide a model for the world's internal brushes
    pub fn set_internal_brushes(&mut self, model: Model) {
        self.internal.internal_ids.remove(self.internal.internal_ids.iter().position(|i| *i == self.internal.brushes).expect("Brushes model was not present"));
        self.internal.brushes = self.insert_model(model);
        self.internal.internal_ids.push(self.internal.brushes);
    }

    pub fn set_arrows_visible(&mut self, visible: bool) {
        self.set_model_visible(self.internal.arrow_nx, visible);
        self.set_model_visible(self.internal.arrow_ny, visible);
        self.set_model_visible(self.internal.arrow_nz, visible);
        self.set_model_visible(self.internal.arrow_px, visible);
        self.set_model_visible(self.internal.arrow_py, visible);
        self.set_model_visible(self.internal.arrow_pz, visible);
    }

    pub fn set_boxes_visible(&mut self, visible: bool) {
        self.set_model_visible(self.internal.box_nx, visible);
        self.set_model_visible(self.internal.box_ny, visible);
        self.set_model_visible(self.internal.box_nz, visible);
        self.set_model_visible(self.internal.box_px, visible);
        self.set_model_visible(self.internal.box_py, visible);
        self.set_model_visible(self.internal.box_pz, visible);
    }

    pub fn debug_arrow(&mut self, start: Vector3<f32>, end: Vector3<f32>) {
        self.set_model_visible(self.internal.debug_arrow, true);
        let dir = (end - start).normalize();
        let length = (end - start).magnitude();
        let rotation = Quaternion::between_vectors(Vector3::unit_y(), dir);

        self.set_model_transform(self.internal.debug_arrow,
            Matrix4::from_translation(start) *
            Matrix4::from(rotation) *
            Matrix4::from_nonuniform_scale(0.5, length / ARROW_HEIGHT, 0.5) *
            Matrix4::from_translation(vec3(0.0, -ARROW_LOWEST_Y, 0.0)) // ????????? 
        );
    }

    /// Hide selection arrows, move them away, hide selection box
    pub fn deselect(&mut self) {
        self.editor_data.selected_object = None;
        self.set_arrows_visible(false);
        self.set_boxes_visible(false);
        self.editor_data.selection_box_visible = false;
        self.move_arrows_far();
        self.move_boxes_far();
        self.editor_data.light_selected = None;
    }

    pub fn set_model_visible(&mut self, model: usize, visible: bool) {
        if let Some(model) = self.models.get(model).as_ref().unwrap() {
            assert!(model.mobile, "Only mobile models can be hidden");
            for (renderable, index) in model.render.iter().zip(model.renderable_indices.iter()) {
                if let Some(mesh) = renderable.get_mesh() {
                    if model.foreground {
                        self.scene.foreground_meshes.get_mut(mesh).unwrap().get_mut(*index).unwrap().draw = visible;
                    } else {
                        self.scene.mobile_meshes.get_mut(mesh).unwrap().get_mut(*index).unwrap().draw = visible;
                    }  
                }
            }
        }
    }

    // https://antongerdelan.net/opengl/raycasting.html
    pub fn get_mouse_ray(&mut self, x: f64, y: f64, window_width: u32, window_height: u32) -> (Vector3<f32>, Vector3<f32>) {
        let x = (2.0 * x as f32) / window_width as f32 - 1.0;
        let y = 1.0 - (2.0 * y as f32) / window_height as f32;
        let ray_clip = vec4(x, y, -1.0, 1.0);
        let mut ray_eye = self.scene.camera.inverse_projection * ray_clip;
        ray_eye.z = -1.0;
        ray_eye.w = 0.0;
        let ray_world = self.scene.camera.inverse_view * ray_eye;
        (self.scene.camera.pos.to_vec(), ray_world.xyz().normalize())
    }

    fn transform_selection(&mut self, new_origin: Vector3<f32>, selection: &Selection) {
        match selection {
            Selection::Brush(brush) => {
                self.set_brush_origin(*brush, new_origin);
            },
            Selection::Model(model) => {
                let transform = self.get_model_transform(*model).unwrap();
                let current_origin = (transform * vec4(0.0, 0.0, 0.0, 1.0)).xyz();
                let new_transform = Matrix4::from_translation(new_origin - current_origin) * transform;
                self.set_model_transform(*model, new_transform);
            },
            Selection::Multiple(_) => unreachable!()
        }
    }

    fn drag_along_axis(&mut self, model_origin: Vector3<f32>, mouse_ray: (Vector3<f32>, Vector3<f32>), axis: Vector3<f32>, plane: Vector3<f32>, axis_func: fn(Vector3<f32>) -> f32) {
        let d = -model_origin.dot(plane); // ????

        let t = -((mouse_ray.0.dot(plane) + d) / mouse_ray.1.dot(plane));

        if t > 0.0 {
            let intersection = mouse_ray.0 + mouse_ray.1 * t; 

            match self.editor_data.init_drag_along_plane {
                Some(pos) => {
                    let diff = intersection - pos;
                    let along_axis = common::round_to(axis_func(diff), self.editor_data.increment);
                    if along_axis.abs_diff_ne(&self.editor_data.drag_distance.unwrap(), EPSILON) {
                        self.editor_data.drag_distance = Some(along_axis);
                        let new_origin = self.editor_data.drag_object_origin.unwrap() + axis * along_axis;

                        let selection = self.editor_data.selected_object.take().unwrap();

                        match &selection {
                            Selection::Brush(_) | Selection::Model(_) => self.transform_selection(new_origin, &selection),
                            Selection::Multiple(multiple) => {
                                for (i, selection) in multiple.iter().enumerate() {
                                    self.transform_selection(new_origin + self.editor_data.multiple_selection_offsets[i], selection);
                                }
                            }
                        }

                        self.editor_data.selected_object = Some(selection);
                    }
                },
                None => { 
                    self.editor_data.init_drag_along_plane = Some(intersection);
                    self.editor_data.drag_distance = Some(0.0);
                    self.editor_data.drag_object_origin = Some(model_origin);
                }
            }
        }
    }

    fn scale_along_axis(&mut self, model_origin: Vector3<f32>, model_scale: Vector3<f32>, mouse_ray: (Vector3<f32>, Vector3<f32>), axis: Vector3<f32>, plane: Vector3<f32>, axis_func: fn(Vector3<f32>) -> f32) {
        let d = -model_origin.dot(plane);
        let t = -((mouse_ray.0.dot(plane) + d) / mouse_ray.1.dot(plane));

        if t > 0.0 {
            let intersection = mouse_ray.0 + mouse_ray.1 * t;

            match self.editor_data.init_drag_along_plane {
                Some(pos) => {
                    let diff = intersection - pos;
                    let along_axis = common::round_to(axis_func(diff), self.editor_data.increment);
                    if along_axis.abs_diff_ne(&self.editor_data.drag_distance.unwrap(), EPSILON) {
                        let old_drag_distance = self.editor_data.drag_distance.unwrap();
                        self.editor_data.drag_distance = Some(along_axis);
                        let (new_scale, new_origin) = if self.editor_data.drag_object_sign.unwrap() {
                            let mut new_scale = self.editor_data.drag_object_scale.unwrap() + axis * along_axis;
                            let mut new_origin = self.editor_data.drag_object_origin.unwrap() + (axis * along_axis / 2.0);

                            if axis_func(new_scale) < self.editor_data.increment {
                                self.editor_data.drag_distance = Some(old_drag_distance);
                                new_scale = self.editor_data.drag_object_scale.unwrap() + axis * old_drag_distance;
                                new_origin = self.editor_data.drag_object_origin.unwrap() + (axis * old_drag_distance / 2.0);
                            }

                            (new_scale, new_origin)
                        } else {
                            let mut new_scale = self.editor_data.drag_object_scale.unwrap() - axis * along_axis;
                            let mut new_origin = self.editor_data.drag_object_origin.unwrap() + (axis * along_axis / 2.0);

                            if axis_func(new_scale) < self.editor_data.increment {
                                self.editor_data.drag_distance = Some(old_drag_distance);
                                new_scale = self.editor_data.drag_object_scale.unwrap() - axis * old_drag_distance;
                                new_origin = self.editor_data.drag_object_origin.unwrap() + (axis * old_drag_distance / 2.0);
                            }

                            (new_scale, new_origin)
                        };

                        match self.editor_data.selected_object.as_ref().unwrap() {
                            Selection::Brush(brush) => {
                                self.set_brush_origin_scale(*brush, new_origin, Some(new_scale));
                            },
                            _ => todo!()
                        }
                    }
                },
                None => {
                    self.editor_data.init_drag_along_plane = Some(intersection);
                    self.editor_data.drag_distance = Some(0.0);
                    self.editor_data.drag_object_origin = Some(model_origin);
                    self.editor_data.drag_object_scale = Some(model_scale);
                }
            }
        }
    }

    fn duplicate_model(&mut self, model: usize) -> usize {
        let model = self.models.get(model).unwrap().as_ref().unwrap();

        let mut new_model = Model {
            transform: model.transform,
            solid: model.solid,
            foreground: model.foreground, 
            extents: model.extents,
            index: None,
            insert_collider: model.insert_collider.clone(),
            colliders: Vec::new(),
            lights: Vec::new(),
            mobile: model.mobile,
            render: model.render.clone(),
            renderable_indices: Vec::new(),
            components: model.components.clone()
        };

        for (offset, i) in model.lights.iter() {
            let cloned_light = self.scene.add_point_light(self.scene.point_lights[*i].clone());
            new_model.lights.push((*offset, cloned_light));
        }

        self.insert_model(new_model)
    }

    pub fn remove_brush(&mut self, brush_index: usize) {
        let brushes = self.models.get_mut(self.internal.brushes).unwrap().as_mut().unwrap();
        self.scene.remove_renderable(brushes, brush_index);
        if let Some(collider) = brushes.colliders[brush_index] {
            self.physical_scene.remove_collider(collider).unwrap();
        }
        
        brushes.colliders.remove(brush_index);

        // This is only local to the brushes model
        for collider in brushes.colliders.iter().flatten() {
            if let Some(collider) = self.physical_scene.colliders.get_mut(*collider).unwrap() {
                if let Some(ref mut renderable) = collider.renderable {
                    if *renderable > brush_index {
                        *renderable -= 1;
                    }
                }
            }
        }
    }

    pub fn update(&mut self, input: &Input, mouse_ray: (Vector3<f32>, Vector3<f32>), delta_time: f32) {
        if self.freeze > 0 {
            self.freeze -= 1;
            return;
        }

        self.player.update(&self.scene.camera, input);

        let mut selection = self.editor_data.selected_object.take();
        let mut selection_changed = false;

        if let Some(selected) = &selection {
            match selected {
                Selection::Brush(brush) => {
                    if let Some(material) = self.editor_data.apply_material.take() {
                        let new_index = self.set_brush_material(*brush, material);
                        self.select_brush(new_index);
                        selection_changed = true;
                    } else {
                        match self.editor_data.selection_type {
                            SelectionType::Movement => {
                                self.adorn_arrows_brush(*brush);
                            },
                            SelectionType::Scaling => {
                                self.adorn_boxes_brush(*brush);
                            },
                            // _ => ()
                        }
                    }
                },
                Selection::Model(model) => {
                    match self.editor_data.selection_type {
                        SelectionType::Movement => {
                            self.adorn_arrows_model(*model);
                        },
                        _ => ()
                    }
                },
                Selection::Multiple(multiple) => {
                    match self.editor_data.selection_type {
                        SelectionType::Movement => {
                            self.adorn_arrows_multiple(multiple);
                        },
                        _ => ()
                    }
                }
            }

            // Delete selected
            if input.get_key_just_pressed(Key::Named(NamedKey::Delete)) || input.get_key_just_pressed(Key::Named(NamedKey::Backspace)) {
                match selected {
                    Selection::Brush(brush) => {
                        self.remove_brush(*brush);
                        self.deselect();
                        selection = None;
                    },
                    Selection::Model(model) => {
                        self.remove_model(*model).unwrap();
                        self.deselect();
                        selection = None;
                    },
                    Selection::Multiple(multiple) => {
                        for multiple_selection in multiple {
                            match multiple_selection {
                                Selection::Brush(brush) => self.remove_brush(*brush),
                                Selection::Model(model) => self.remove_model(*model).unwrap(),
                                Selection::Multiple(_) => unreachable!()
                            }
                        }
                        self.deselect();
                        selection = None;
                    }
                }
            }
        }
        if !selection_changed {
            self.editor_data.selected_object = selection;
        }
        self.editor_data.apply_material = None;

        // Duplicate
        if input.get_key_pressed(Key::Named(NamedKey::Control)) && input.get_key_just_pressed(Key::Character("d".into())) {
            if self.editor_data.selected_object.is_some() {
                let selection = self.editor_data.selected_object.take().unwrap();

                let mut new_selection: Option<Vec<Selection>> = None;
                match &selection {
                    Selection::Brush(brush_index) => {
                        let brush = self.models.get(self.internal.brushes).unwrap().as_ref().unwrap().render[*brush_index].clone();
                        let index = self.insert_brush(brush);
                        self.select_brush(index);
                    },
                    Selection::Model(model) => {
                        let duplicate = self.duplicate_model(*model);
                        self.select_model(duplicate);
                    },
                    Selection::Multiple(multiple) => {
                        if !multiple.is_empty() {
                            new_selection = Some(Vec::new());
                        }
                        for selection in multiple {
                            match selection {
                                Selection::Brush(brush) => {
                                    let brush = self.models[self.internal.brushes].as_ref().unwrap().render[*brush].clone();
                                    let index = self.insert_brush(brush);
                                    new_selection.as_mut().unwrap().push(Selection::Brush(index));
                                },
                                Selection::Model(model) => {
                                    let duplicate = self.duplicate_model(*model);
                                    new_selection.as_mut().unwrap().push(Selection::Model(duplicate));
                                },
                                Selection::Multiple(_) => unreachable!()
                            }
                        }
                    }
                }

                if let Some(new) = new_selection {
                    self.deselect();
                    for selection in new {
                        match selection {
                            Selection::Brush(brush) => self.select_or_append_brush(brush),
                            Selection::Model(model) => self.select_or_append_model(model),
                            Selection::Multiple(_) => unreachable!()
                        }
                        self.set_arrows_visible(true);
                    }
                }

                self.editor_data.selected_object = Some(selection);
            }
        }

        // Disable dragging if lmb is let go
        if self.editor_data.drag_axis.is_some() && input.get_mouse_button_released(MouseButton::Left) {
            self.editor_data.drag_axis = None;
            self.editor_data.drag_object_origin = None;
            self.editor_data.drag_object_scale = None;
            self.editor_data.drag_distance = None;
            self.editor_data.init_drag_along_plane = None;
        }

        // https://antongerdelan.net/opengl/raycasting.html
        // Do a ray-plane intersection test and take the component of the vector from the original click to the current click that lies on the proper axis
        // The plane is the normal * sign(d), d is the distance from the origin to the object along the respective axis
        if let Some(drag) = &self.editor_data.drag_axis {
            if self.editor_data.selected_object.is_some() {
                let (model_origin, model_scale) = match self.editor_data.selected_object.as_ref().unwrap() {
                    Selection::Brush(brush) => {
                        // small performance improvement possible
                        let (pos, scale) = self.get_brush_adornment_transform(*brush);
                        (pos, (scale - vec3(1.0, 1.0, 1.0)) * 2.0)
                    },
                    Selection::Model(model) => {
                        // model_scale goes unused for now
                        ((self.models.get(*model).unwrap().as_ref().unwrap().transform * vec4(0.0, 0.0, 0.0, 1.0)).xyz(), Vector3::zero())
                    },
                    Selection::Multiple(_) => {
                        (self.editor_data.selection_box_pos, self.editor_data.selection_box_scale)
                    }
                };

                // t < 0, intersects behind ray. t == 0, ray is perpendicular 
                match self.editor_data.selection_type {
                    SelectionType::Movement => {
                        match drag {
                            DragAxis::X => self.drag_along_axis(model_origin, mouse_ray, Vector3::unit_x(), self.editor_data.drag_plane.unwrap(), |v| v.x),
                            DragAxis::Y => self.drag_along_axis(model_origin, mouse_ray, Vector3::unit_y(), self.editor_data.drag_plane.unwrap(), |v| v.y),
                            DragAxis::Z => self.drag_along_axis(model_origin, mouse_ray, Vector3::unit_z(), self.editor_data.drag_plane.unwrap(), |v| v.z),
                        }
                    },
                    SelectionType::Scaling => {
                        match drag {
                            DragAxis::X => self.scale_along_axis(model_origin, model_scale, mouse_ray, Vector3::unit_x(), self.editor_data.drag_plane.unwrap(), |v| v.x),
                            DragAxis::Y => self.scale_along_axis(model_origin, model_scale, mouse_ray, Vector3::unit_y(), self.editor_data.drag_plane.unwrap(), |v| v.y),
                            DragAxis::Z => self.scale_along_axis(model_origin, model_scale, mouse_ray, Vector3::unit_z(), self.editor_data.drag_plane.unwrap(), |v| v.z),
                        }
                    },
                    // _ => ()
                }
            } else {
                eprintln!("Drag started without a selection");
            }
        }

        match self.player.movement {
            PlayerMovementMode::FirstPerson => {
                self.player.velocity += -Vector3::unit_y() * (self.gravity * delta_time);
                let result = self.physical_scene.move_and_slide(self.player.collider, self.player.velocity * delta_time);
                self.player.position = result.final_position;
                self.player.velocity = result.velocity / delta_time;

                let mut grounded = false;
                let mut ground = None;
                for (i, normal) in result.normals.iter().enumerate() {
                    if normal.normalize().dot(Vector3::unit_y()) > 0.75 {
                        grounded = true;
                        ground = Some(*result.materials.get(i).unwrap());
                        break;
                    }
                }
                if grounded {
                    self.player.velocity *= ground.unwrap().friction;
                    self.player.ground = ground;
                    self.player.coyote = COYOTE;
                } else {
                    self.player.velocity *= self.air_friction;
                }

                self.scene.camera.pos = Point3::from_vec(self.player.position + vec3(0.0, 0.5, 0.0));
            },
            PlayerMovementMode::FollowCamera => {
                self.player.position = self.scene.camera.pos.to_vec();
                self.physical_scene.set_collider_pos(self.player.collider, self.player.position);
                self.player.velocity = Vector3::zero()
            }
        }

        for i in 0..self.models.len() {
            if self.models[i].is_some() {
                let mut model = self.models[i].take().unwrap();

                for j in 0..model.components.len() {
                    model = Component::on_update(j, model, self);
                }

                self.models[i] = Some(model);
            }
        }
    }

    pub unsafe fn load_basic_meshes(meshes: &mut MeshBank, gl: &glow::Context) {
        meshes.add(Mesh::create_square(0.3, 0.2, 0.1, gl), "square");
        meshes.add(Mesh::create_material_square("test", gl), "square_textured");
        meshes.add(Mesh::create_material_cube("test", gl), "cube");
        meshes.add(Mesh::create_cube(gl), "blank_cube");
    }
}

#[derive(Clone, Debug)]
pub enum Renderable {
    Mesh(String, Matrix4<f32>, u32),
    Brush(String, Vector3<f32>, Vector3<f32>, u32),
    Billboard(String, Vector3<f32>, (f32, f32), u32, bool)
}

impl Renderable {
    pub fn get_mesh(&self) -> Option<&str> {
        match self {
            Self::Mesh(s, _, _) => Some(s),
            Self::Brush(s, _, _, _) => Some(s),
            _ => None
        }
    }

    pub fn get_extents(&self) -> Option<parry3d::bounding_volume::Aabb> {
        match self {
            Self::Brush(_, pos, extents, _) => {
                Some(parry3d::bounding_volume::Aabb::from_half_extents(
                    parry3d::na::Point3::new(pos.x, pos.y, pos.z), 
                    parry3d::na::Vector3::new(extents.x / 2.0, extents.y / 2.0, extents.z / 2.0)
                ))
            },
            _ => None
        }
    }

    pub fn render_as_mesh(&self) -> bool {
        matches!(self, Self::Mesh(..) | Self::Brush(..))
    }
}

// TODO: PhysicalProperties
#[derive(Clone)]
pub enum ModelCollider {
    Cuboid { offset: Vector3<f32>, half_extents: Vector3<f32> },
    Multiple { colliders: Vec<ModelCollider> }
}

#[derive(Clone)]
pub struct Model {
    pub transform: Matrix4<f32>,
    pub render: Vec<Renderable>,
    pub mobile: bool,
    pub foreground: bool,
    pub renderable_indices: Vec<usize>,
    /// 0 -> #`render` all correspond with eachother, onwards is from `insert_collider`
    pub colliders: Vec<Option<usize>>,
    pub index: Option<usize>,
    pub insert_collider: Option<ModelCollider>,
    pub solid: bool,
    /// offset, half extents
    pub extents: Option<(Vector3<f32>, Vector3<f32>)>,
    pub lights: Vec<(Vector3<f32>, usize)>,
    pub components: Vec<Component>
}

impl Model {
    pub fn new(mobile: bool, transform: Matrix4<f32>, renderables: Vec<Renderable>) -> Self {
        Self {
            transform,
            render: renderables,
            mobile,
            renderable_indices: Vec::new(),
            colliders: Vec::new(),
            index: None,
            insert_collider: None,
            foreground: false,
            solid: true,
            extents: None,
            lights: Vec::new(),
            components: Vec::new()
        }
    }

    pub fn from_loaded_file(file: &str, meshes: &MeshBank) -> Option<Self> {
        let mut current_index = 0;

        meshes.get(&format!("File_{}0", file))?;

        let mut model = Self {
            mobile: false,
            render: Vec::new(),
            renderable_indices: Vec::new(),
            transform: Matrix4::identity(),
            colliders: Vec::new(),
            index: None,
            insert_collider: None,
            foreground: false,
            solid: true,
            extents: None,
            lights: Vec::new(),
            components: Vec::new()
        };

        while meshes.get(&format!("File_{}{}", file, current_index)).is_some() {
            model.render.push(Renderable::Mesh(format!("File_{}{}", file, current_index), Matrix4::identity(), 0));
            current_index += 1;
        }

        Some(model)
    }

    pub fn origin(&self) -> Vector3<f32> {
        vec3(self.transform.w.x, self.transform.w.y, self.transform.w.z)
    }

    pub fn mobile(mut self) -> Self {
        self.mobile = true;
        self
    }

    pub fn foreground(mut self) -> Self {
        // foreground implies mobile
        self.foreground = true;
        self.mobile = true;
        self
    }

    pub fn fullbright(mut self) -> Self {
        for renderable in self.render.iter_mut() {
            match renderable {
                Renderable::Brush(_, _, _, flags) => *flags |= flags::FULLBRIGHT,
                Renderable::Mesh(_, _, flags) => *flags |= flags::FULLBRIGHT,
                Renderable::Billboard(_, _, _, flags, _) => *flags |= flags::FULLBRIGHT
            }
        }
        self
    }

    pub fn with_component(mut self, component: Component) -> Self {
        self.components.push(component);
        self
    }

    pub fn transform(mut self, append: Matrix4<f32>) -> Self {
        self.transform = append * self.transform;
        self
    }

    pub fn translate(self, by: Vector3<f32>) -> Self {
        self.transform(Matrix4::from_translation(by))
    }

    pub fn scale(self, by: f32) -> Self {
        self.transform(Matrix4::from_scale(by))
    }

    fn insert_collider(&mut self, collider: ModelCollider) {
        match &mut self.insert_collider {
            Some(ModelCollider::Multiple { colliders }) => {
                colliders.push(collider);
            }
            None => {
                self.insert_collider = Some(collider);
            }
            Some(_) => {
                let singular = self.insert_collider.take().unwrap();
                self.insert_collider = Some(ModelCollider::Multiple { colliders: vec![
                    singular, collider
                ] })
            }
        }
    }

    pub fn collider_cuboid(mut self, offset: Vector3<f32>, half_extents: Vector3<f32>) -> Self {
        self.insert_collider(ModelCollider::Cuboid { offset, half_extents });
        self
    }

    pub fn non_solid(mut self) -> Self {
        self.solid = false;
        self
    }

    pub fn with_light(mut self, index: usize, position: Vector3<f32>) -> Self {
        self.lights.push((position, index));
        self
    }
}

#[derive(Clone)]
pub enum PlayerMovementMode {
    FollowCamera,
    FirstPerson
}

pub struct Player {
    pub collider: usize,
    pub position: Vector3<f32>,
    pub velocity: Vector3<f32>,
    pub speed: f32,
    pub jump_velocity: f32,
    pub movement: PlayerMovementMode,
    pub ground: Option<PhysicalProperties>,
    pub air_control: f32,
    pub coyote: u32
}

impl Player {
    pub fn new() -> Self { 
        Self {
            collider: 0,
            position: vec3(0.0, 0.0, 0.0),
            velocity: Vector3::zero(),
            jump_velocity: 7.0,
            speed: 5.0,
            movement: PlayerMovementMode::FirstPerson,
            ground: None,
            air_control: 0.01,
            coyote: 0
        }
    }

    fn control(&self) -> f32 {
        if self.coyote > 0 {
            if let Some(ground) = self.ground {
                ground.control
            } else {
                self.air_control
            }
        } else {
            self.air_control
        }
    }

    pub fn update(&mut self, camera: &Camera, input: &Input) {
        match self.movement {
            PlayerMovementMode::FirstPerson => {
                let norm_dir = camera.direction.normalize();
                let projected_forward = vec3(norm_dir.x, 0.0, norm_dir.z);
                let mut movement_vector = Vector3::zero();
                let control = self.control();
                if !input.get_key_pressed(Key::Named(NamedKey::Control)) {
                    if input.get_key_pressed(Key::Character("w".into())) {
                        movement_vector += projected_forward.normalize();
                    }
                    if input.get_key_pressed(Key::Character("s".into())) {
                        movement_vector -= projected_forward.normalize();
                    }
                    if input.get_key_pressed(Key::Character("a".into())) {
                        movement_vector += camera.up.cross(camera.direction).normalize().mul_element_wise(vec3(1.0, 0.0, 1.0));
                    }
                    if input.get_key_pressed(Key::Character("d".into())) {
                        movement_vector -= camera.up.cross(camera.direction).normalize().mul_element_wise(vec3(1.0, 0.0, 1.0));
                    }
                }

                if movement_vector.magnitude2() > 0.01 {
                    let desired_velocity = movement_vector.normalize() * self.speed;
                    let controlled_velocity = desired_velocity * control;

                    if desired_velocity.x < 0.0 {
                        if self.velocity.x > desired_velocity.x {
                            self.velocity.x += controlled_velocity.x;
                            self.velocity.x = self.velocity.x.max(desired_velocity.x);
                        }
                    } else if desired_velocity.x > 0.0 {
                        if self.velocity.x < desired_velocity.x {
                            self.velocity.x += controlled_velocity.x;
                            self.velocity.x = self.velocity.x.min(desired_velocity.x);
                        }
                    }

                    if desired_velocity.z < 0.0 {
                        if self.velocity.z > desired_velocity.z {
                            self.velocity.z += controlled_velocity.z;
                            self.velocity.z = self.velocity.z.max(desired_velocity.z);
                        }
                    } else if desired_velocity.z > 0.0 {
                        if self.velocity.z < desired_velocity.z {
                            self.velocity.z += controlled_velocity.z;
                            self.velocity.z = self.velocity.z.min(desired_velocity.z);
                        }
                    }
                }

                if self.coyote > 0 {
                    if input.get_key_just_pressed(Key::Named(NamedKey::Space)) {
                        self.velocity.y = self.jump_velocity * self.ground.map(|s| s.jump).unwrap_or(1.0);
                    }
                    self.coyote -= 1;
                }
            },
            PlayerMovementMode::FollowCamera => ()
        }
    }
}