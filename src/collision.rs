use cgmath::{vec3, InnerSpace, Vector3};
use parry3d::{bounding_volume::{Aabb, BoundingSphere, BoundingVolume}, na::{Isometry3, OPoint, Point3}, query::{self, Contact}, shape::{Ball, Cuboid, Shape}};

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
            if let None = maybe_empty {
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

    // pub fn move_and_slide(&mut self, index: usize, vel: Vector3<f32>) -> Vector3<f32> {
    //     self.colliders.get_mut(index).unwrap().shift(vel.x, vel.y, vel.z);
    //     let mut final_velocity = vel;

    //     for (i, collider) in self.colliders.iter().enumerate() {
    //         if i != index {
    //             if let Some(contact) = self.colliders.get(index).unwrap().get_contact(collider) {
    //                 let hit_normal = vec3(contact.normal2.x, contact.normal2.y, contact.normal2.z);
    //                 let projected = final_velocity.project_on(hit_normal);
    //                 final_velocity = vel - projected;

    //                 self.colliders.get_mut(index).unwrap().shift(-vel.x, -vel.y, -vel.z);
    //                 self.colliders.get_mut(index).unwrap().shift(final_velocity.x, final_velocity.y, final_velocity.z);
    //             }
    //         }
    //     }

    //     final_velocity
    // }

    pub fn move_and_slide(&mut self, index: usize, vel: Vector3<f32>) -> MoveSlideResult {
        self.colliders.get_mut(index).unwrap().as_mut().unwrap().shift(vel.x, vel.y, vel.z);
        let mut final_velocity = vel;
        let mut normals = Vec::new();
        let mut materials = Vec::new();

        for i in 0..self.colliders.len() {
            if i != index {
                if let Some(contact) = self.colliders.get(index).unwrap().as_ref().unwrap().get_contact(self.colliders.get(i).unwrap().as_ref().unwrap()) {
                    let initial_velocity = final_velocity;
                    let hit_normal = vec3(contact.normal2.x, contact.normal2.y, contact.normal2.z);
                    // If self is already inside other dont do anything
                    if hit_normal.normalize().dot(final_velocity.normalize()) < 0.0 {
                        // Stairs check
                        let mut skip_resolve = false;
                        if hit_normal.y.abs() < 0.01 {
                            let this_bounding = self.colliders.get(index).unwrap().as_ref().unwrap().bounding;
                            let other_bounding = self.colliders.get(i).unwrap().as_ref().unwrap().bounding;
                            let standing_diff = (other_bounding.center().y + other_bounding.half_extents().y) - (this_bounding.center().y - this_bounding.half_extents().y);
                            if standing_diff < STAIR_MAX_SIZE {
                                // final_velocity.y += standing_diff;
                                // self.colliders.get_mut(index).unwrap().as_mut().unwrap().shift(-initial_velocity.x, -initial_velocity.y, -initial_velocity.z);
                                // self.colliders.get_mut(index).unwrap().as_mut().unwrap().shift(final_velocity.x, final_velocity.y, final_velocity.z);
                                self.colliders.get_mut(index).unwrap().as_mut().unwrap().shift(0.0, standing_diff, 0.0);
                                skip_resolve = true;
                            }
                        }

                        if !skip_resolve {
                            let projected = final_velocity.project_on(hit_normal);
                            final_velocity = final_velocity - projected;
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
}

pub struct MoveSlideResult {
    pub velocity: Vector3<f32>,
    // Normals of each object collided with
    pub normals: Vec<Vector3<f32>>,
    // Corresponds to normals
    pub materials: Vec<PhysicalProperties>,
    pub final_position: Vector3<f32>
}

#[derive(Clone, Copy, Debug)]
pub struct PhysicalProperties {
    pub friction: f32,
    pub control: f32
}

impl Default for PhysicalProperties {
    fn default() -> Self {
        Self {
            friction: 0.80,
            control: 1.0
        }
    }
}

pub struct Collider {
    pub bounding: Aabb,
    pub shape: Box<dyn Shape>,
    pub pos: Isometry3<f32>,
    pub physical_properties: PhysicalProperties
}

impl Collider {
    pub fn shift(&mut self, dx: f32, dy: f32, dz: f32) {
        self.pos.translation.x += dx;
        self.pos.translation.y += dy;
        self.pos.translation.z += dz;
        self.bounding = self.bounding.translated(&parry3d::na::Vector3::new(dx, dy, dz));
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
            physical_properties: PhysicalProperties::default()
        }
    }

    pub fn set_friction(mut self, friction: f32) {
        self.physical_properties.friction = friction;
    }
}