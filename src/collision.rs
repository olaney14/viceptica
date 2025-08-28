use core::f32;

use cgmath::{vec3, vec4, InnerSpace, Vector3, Zero};
use parry3d::{bounding_volume::{Aabb, BoundingVolume}, na::{Isometry3, Point3}, query::{self, Contact, Ray}, shape::{Cuboid, Shape}};
use serde::{Deserialize, Serialize};

use crate::world::{Model, ModelCollider, Renderable, World};

pub const STAIR_MAX_SIZE: f32 = 0.55;

pub struct PhysicalScene {
    pub colliders: Vec<Option<Collider>>
}

impl PhysicalScene {
    pub fn new() -> Self {
        Self {
            colliders: Vec::new()
        }
    }

    pub fn add_collider(&mut self, collider: Collider) -> usize {
        for (i, maybe_empty) in self.colliders.iter_mut().enumerate() {
            if maybe_empty.is_none() {
                *maybe_empty = Some(collider);
                return i;
            }
        }

        self.colliders.push(Some(collider));
        self.colliders.len() - 1
    }

    pub fn remove_collider(&mut self, index: usize) -> Result<(), String> {
        if let Some(collider) = self.colliders.get_mut(index) {
            *collider = None;
            Ok(())
        } else {
            Err("Index out of bounds".to_string())
        }
    }

    pub fn set_collider_pos(&mut self, index: usize, pos: Vector3<f32>) {
        self.colliders.get_mut(index).unwrap().as_mut().unwrap().set_pos(pos.x, pos.y, pos.z);
    }

    pub fn move_and_slide(&mut self, index: usize, vel: Vector3<f32>) -> MoveSlideResult {
        self.colliders.get_mut(index).unwrap().as_mut().unwrap().shift(vel.x, vel.y, vel.z);
        let mut final_velocity = vel;
        let mut normals = Vec::new();
        let mut materials = Vec::new();

        for i in 0..self.colliders.len() {
            if i != index {
                if self.colliders[i].is_none() { continue; }
                if !self.colliders.get(i).unwrap().as_ref().unwrap().solid { continue; }
                if let Some(contact) = self.colliders.get(index).unwrap().as_ref().unwrap().get_contact(self.colliders.get(i).unwrap().as_ref().unwrap()) {
                    let initial_velocity = final_velocity;
                    let hit_normal = vec3(contact.normal2.x, contact.normal2.y, contact.normal2.z);
                    // If self is already inside other dont do anything
                    if hit_normal.normalize().dot(final_velocity.normalize()) < 0.0 {
                        // Stairs check
                        let mut skip_resolve = false;
                        if hit_normal.y.abs() < 0.01 && vel.y < 0.005 {
                            let this_bounding = self.colliders.get(index).unwrap().as_ref().unwrap().bounding;
                            let other_bounding = self.colliders.get(i).unwrap().as_ref().unwrap().bounding;
                            let standing_diff = (other_bounding.center().y + other_bounding.half_extents().y) - (this_bounding.center().y - this_bounding.half_extents().y);
                            if standing_diff < STAIR_MAX_SIZE {
                                self.colliders.get_mut(index).unwrap().as_mut().unwrap().shift(0.0, standing_diff, 0.0);
                                skip_resolve = true;
                            }
                        }

                        if !skip_resolve {
                            let projected = final_velocity.project_on(hit_normal);
                            final_velocity -= projected;
                            normals.push(hit_normal);
                            materials.push(self.colliders.get(i).unwrap().as_ref().unwrap().physical_properties);

                            self.colliders.get_mut(index).unwrap().as_mut().unwrap().shift(-initial_velocity.x, -initial_velocity.y, -initial_velocity.z);
                            self.colliders.get_mut(index).unwrap().as_mut().unwrap().shift(final_velocity.x, final_velocity.y, final_velocity.z);
                        }
                    }
                }
            }
        }

        let pos = self.colliders.get(index).unwrap().as_ref().unwrap().pos;
        let final_position = vec3(pos.translation.x, pos.translation.y, pos.translation.z);

        MoveSlideResult {
            velocity: final_velocity,
            normals,
            materials,
            final_position
        }
    }

    pub fn raycast(&mut self, origin: Vector3<f32>, direction: Vector3<f32>, distance: f32, params: &RaycastParameters) -> Option<RaycastResult> {
        let mut closest_intersection = f32::MAX;
        let mut result: Option<RaycastResult> = None;
        let ray = Ray::new(Point3::new(origin.x, origin.y, origin.z), parry3d::na::Vector3::new(direction.x, direction.y, direction.z).normalize());
        
        // Check for meshes in the foreground (e.g. movement arrows)
        if params.select_foreground {
            for i in 0..self.colliders.len() {
                if params.ignore.contains(&i) { continue; }
                
                if let Some(collider) = &self.colliders[i] {
                    if collider.foreground {
                        if params.respect_solid && !collider.solid { continue; }
                        if let Some(intersection) = collider.shape.cast_ray_and_get_normal(&collider.pos, &ray, distance, true) {
                            if intersection.time_of_impact < closest_intersection {
                                closest_intersection = intersection.time_of_impact;
                                let intersection_pos = origin + direction.normalize() * intersection.time_of_impact;
                                result = Some(RaycastResult {
                                    normal: vec3(intersection.normal.x, intersection.normal.y, intersection.normal.z),
                                    pos: intersection_pos,
                                    model: collider.model,
                                    renderable: collider.renderable
                                });
                            }
                        }
                    }
                }
            }

            // If the foreground check doesnt yield anything move on to the rest of the colliders
            if result.is_some() {
                return result;
            }
        }

        for i in 0..self.colliders.len() {
            if params.ignore.contains(&i) { continue; }

            if let Some(collider) = &self.colliders[i] {
                if params.respect_solid && !collider.solid { continue; }
                if let Some(intersection) = collider.shape.cast_ray_and_get_normal(&collider.pos, &ray, distance, true) {
                    if intersection.time_of_impact < closest_intersection {
                        closest_intersection = intersection.time_of_impact;
                        let intersection_pos = origin + direction.normalize() * intersection.time_of_impact;
                        result = Some(RaycastResult {
                            normal: vec3(intersection.normal.x, intersection.normal.y, intersection.normal.z),
                            pos: intersection_pos,
                            model: collider.model,
                            renderable: collider.renderable
                        });
                    }
                }
            }
        }

        result
    }
}

#[derive(Debug)]
pub struct RaycastResult {
    pub pos: Vector3<f32>,
    pub normal: Vector3<f32>,
    pub model: Option<usize>,
    pub renderable: Option<usize>
}

pub struct RaycastParameters {
    pub ignore: Vec<usize>,
    pub select_foreground: bool,
    pub respect_solid: bool
}

impl RaycastParameters {
    pub fn new() -> Self {
        Self {
            ignore: Vec::new(),
            respect_solid: false,
            select_foreground: false
        }
    }

    pub fn ignore(mut self, ignore: Vec<usize>) -> Self {
        self.ignore = ignore;
        self
    }

    pub fn respect_solid(mut self) -> Self {
        self.respect_solid = true;
        self
    }

    pub fn select_foreground(mut self) -> Self {
        self.select_foreground = true;
        self
    }
}

impl Model {
    fn insert_model_collider(&mut self, model_collider: &ModelCollider, model_position: Vector3<f32>, world: &mut World) {
        let mut extents: Option<Aabb> = None;
        match model_collider {
            ModelCollider::Cuboid { offset, half_extents } => {
                let mut collider = Collider::cuboid(*offset + model_position, *half_extents * 2.0, Vector3::zero());
                collider.physical_properties = PhysicalProperties::default();
                collider.renderable = None;
                collider.model = self.index;
                collider.foreground = self.foreground;
                collider.solid = self.solid;

                if let Some(extents) = &mut extents {
                    extents.merge(&collider.bounding);
                } else {
                    extents = Some(collider.bounding);
                }

                self.colliders.push(Some(world.physical_scene.add_collider(collider)));
            },
            ModelCollider::Multiple { colliders } => {
                for collider in colliders.iter() {
                    self.insert_model_collider(collider, model_position, world);
                }
            }
        }

        if let Some(extents) = extents {
            self.extents = Some((
                vec3(extents.center().x, extents.center().y, extents.center().z) - model_position,
                vec3(extents.half_extents().x, extents.half_extents().y, extents.half_extents().z)
            ));
        }
    }

    pub fn insert_colliders(&mut self, world: &mut World) {
        assert!(self.colliders.is_empty(), "Colliders inserted more than once");
        let model_position: Vector3<f32> = (self.transform * vec4(0.0, 0.0, 0.0, 1.0)).xyz();

        for (i, renderable) in self.render.iter().enumerate() {
            match renderable {
                Renderable::Brush(material, position, size, _) => {
                    let properties = world.scene.materials.get(material).unwrap().physical_properties;
                    let mut collider = Collider::cuboid(*position + model_position, *size, Vector3::zero());
                    collider.physical_properties = properties;
                    collider.renderable = Some(i);
                    collider.model = self.index;
                    collider.foreground = self.foreground;
                    collider.solid = self.solid;
                    self.colliders.push(Some(world.physical_scene.add_collider(collider)));
                },
                _ => {
                    self.colliders.push(None);
                }
            }
        }

        if self.insert_collider.is_some() {
            let insert_collider = self.insert_collider.take().unwrap();
            self.insert_model_collider(&insert_collider, model_position, world);
            self.insert_collider = Some(insert_collider);
        }
    }
}

impl World {
    fn update_model_collider(&mut self, model_collider: &ModelCollider, model_position: Vector3<f32>, model: usize, i: usize) {
        match model_collider {
            ModelCollider::Cuboid { offset, half_extents } => {
                let mut collider = Collider::cuboid(*offset + model_position, *half_extents * 2.0, Vector3::zero());
                collider.physical_properties = PhysicalProperties::default();
                collider.renderable = None;
                collider.model = Some(model);
                collider.foreground = self.models[model].as_ref().unwrap().foreground;
                collider.solid = self.models[model].as_ref().unwrap().solid;
                let collider_index = self.models[model].as_ref().unwrap().colliders[i].unwrap();
                self.physical_scene.colliders[collider_index] = Some(collider);
            },
            ModelCollider::Multiple { colliders } => {
                for (j, collider) in colliders.iter().enumerate() {
                    self.update_model_collider(collider, model_position, model, i + j);
                }
            }
        }
    }

    pub fn update_colliders(&mut self, model: usize) {
        let model_position: Vector3<f32> = (self.models[model].as_ref().unwrap().transform * vec4(0.0, 0.0, 0.0, 1.0)).xyz();

        for i in 0..self.models[model].as_ref().unwrap().render.len() {
            if let Renderable::Brush(material, position, size, _) = &self.models[model].as_ref().unwrap().render[i] {
                let properties = self.scene.materials.get(material).unwrap().physical_properties;
                let mut collider = Collider::cuboid(*position + model_position, *size, Vector3::zero());
                collider.physical_properties = properties;
                collider.renderable = Some(i);
                collider.model = Some(model);
                collider.foreground = self.models[model].as_ref().unwrap().foreground;
                collider.solid = self.models[model].as_ref().unwrap().solid;
                let collider_index = self.models[model].as_ref().unwrap().colliders[i].unwrap();
                self.physical_scene.colliders[collider_index] = Some(collider);
            }
        }

        if self.models[model].as_ref().unwrap().insert_collider.is_some() {
            let insert_collider = self.models[model].as_mut().unwrap().insert_collider.take().unwrap();
            self.update_model_collider(&insert_collider, model_position, model, self.models[model].as_ref().unwrap().render.len());
            self.models[model].as_mut().unwrap().insert_collider = Some(insert_collider);
        }
    }
}

pub struct MoveSlideResult {
    pub velocity: Vector3<f32>,
    // Normals of each object collided with
    pub normals: Vec<Vector3<f32>>,
    // Corresponds to normals
    pub materials: Vec<PhysicalProperties>,
    pub final_position: Vector3<f32>
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct PhysicalProperties {
    pub friction: f32,
    pub control: f32,
    #[serde(default)]
    pub jump: f32
}

impl Default for PhysicalProperties {
    fn default() -> Self {
        Self {
            friction: 0.80,
            control: 1.0,
            jump: 1.0
        }
    }
}

pub struct Collider {
    pub bounding: Aabb,
    pub solid: bool,

    /// Only affects mouse raycasting, if this is true raycast will prioritize this
    pub foreground: bool,
    pub shape: Box<dyn Shape>,
    pub pos: Isometry3<f32>,
    pub physical_properties: PhysicalProperties,
    pub model: Option<usize>,
    pub renderable: Option<usize>
}

impl Collider {
    pub fn shift(&mut self, dx: f32, dy: f32, dz: f32) {
        self.pos.translation.x += dx;
        self.pos.translation.y += dy;
        self.pos.translation.z += dz;
        self.bounding = self.bounding.translated(&parry3d::na::Vector3::new(dx, dy, dz));
    }

    pub fn set_pos(&mut self, x: f32, y: f32, z: f32) {
        let dx = x - self.pos.translation.x;
        let dy = y - self.pos.translation.y;
        let dz = z - self.pos.translation.z;
        self.shift(dx, dy, dz);
    }

    pub fn get_contact(&self, other: &Collider) -> Option<Contact> {
        if self.bounding.intersects(&other.bounding) {
            let contact = query::contact(&self.pos, self.shape.as_ref(), &other.pos, other.shape.as_ref(), 1.0).unwrap();
            if let Some(contact) = contact {
                if contact.dist < 0.0 {
                    return Some(contact);
                }
            }
        }

        None
    }

    pub fn cuboid(center: Vector3<f32>, full_extents: Vector3<f32>, axis_angle: Vector3<f32>) -> Self {
        Self {
            bounding: Aabb::from_half_extents(
                Point3::new(center.x, center.y, center.z), 
                parry3d::na::Vector3::new(full_extents.x / 2.0, full_extents.y / 2.0, full_extents.z / 2.0)
            ).scaled_wrt_center(&parry3d::na::Vector3::new(1.02, 1.02, 1.02)),
            shape: Box::new(Cuboid::new(parry3d::na::Vector3::new(full_extents.x / 2.0, full_extents.y / 2.0, full_extents.z / 2.0))),
            pos: Isometry3::new(
                parry3d::na::Vector3::new(center.x, center.y, center.z),
                parry3d::na::Vector3::new(axis_angle.x, axis_angle.y, axis_angle.z)
            ),
            physical_properties: PhysicalProperties::default(),
            model: None,
            renderable: None,
            foreground: false,
            solid: true
        }
    }

    pub fn set_friction(mut self, friction: f32) {
        self.physical_properties.friction = friction;
    }
}