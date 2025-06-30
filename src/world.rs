use cgmath::{vec3, ElementWise, EuclideanSpace, InnerSpace, Matrix4, Point3, SquareMatrix, Vector3, Zero};
use winit::keyboard::{Key, NamedKey};

use crate::{collision::{Collider, PhysicalProperties, PhysicalScene}, input::Input, mesh::{Mesh, MeshBank}, render::{self, Camera, Scene}, texture::TextureBank};

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

pub struct World {
    models: Vec<Model>,
    pub scene: render::Scene,
    pub player: Player,
    pub physical_scene: PhysicalScene,
    pub gravity: f32,
    pub air_friction: f32
}

pub unsafe fn load_brushes(textures: &mut TextureBank, meshes: &mut MeshBank, scene: &mut Scene, gl: &glow::Context) {
    for (i, texture) in BRUSH_TEXTURES.iter().enumerate() {
        // textures.load_by_name(&texture, gl).unwrap();
        scene.load_material_diff_spec(&texture, &texture, &format!("{}_specular", texture), textures, gl);
        meshes.add(Mesh::create_material_cube(&texture, gl), &format!("Brush_{}", texture));
    }

    scene.load_material_diff_spec_phys("ice", "ice", "ice_specular", PhysicalProperties {
        friction: 0.99,
        control: 0.05
    }, textures, gl);
    meshes.add(Mesh::create_material_cube("ice", gl), "Brush_ice");
    scene.load_material_diff_spec_phys("tar", "tar", "tar_specular", PhysicalProperties {
        friction: 0.25,
        control: 0.03
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
            air_friction: 0.995
        };

        world.player.collider = world.physical_scene.add_collider(Collider::cuboid(Vector3::zero(), vec3(0.5, 2.0, 0.5), Vector3::zero()));

        world
    }

    pub fn insert_model(&mut self, mut model: Model) {
        for renderable in model.render.iter() {
            match renderable {
                Renderable::Brush(material, position, size, _) => {
                    let properties = self.scene.materials.get(material).unwrap().physical_properties;
                    let mut collider = Collider::cuboid(*position, *size, Vector3::zero());
                    collider.physical_properties = properties;
                    self.physical_scene.add_collider(collider);
                },
                _ => ()
            }
        }

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

    pub fn update(&mut self, input: &Input, delta_time: f32) {
        self.player.update(&self.scene.camera, input);

        match self.player.movement {
            PlayerMovementMode::FirstPerson => {
                self.player.velocity += -Vector3::unit_y() * (self.gravity * delta_time);
                let result = self.physical_scene.move_and_slide(self.player.collider, self.player.velocity * delta_time);
                self.player.position = result.final_position;
                //self.player.position += result.velocity; // result.velocity is in units/frame
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
                } else {
                    self.player.velocity *= self.air_friction;
                }
                self.player.ground = ground;

                self.scene.camera.pos = Point3::from_vec(self.player.position + vec3(0.0, 0.5, 0.0));
            },
            PlayerMovementMode::FollowCamera => {
                self.player.position = self.scene.camera.pos.to_vec();
            }
        }
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
    pub air_control: f32
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
            air_control: 0.01
        }
    }

    pub fn update(&mut self, camera: &Camera, input: &Input) {
        match self.movement {
            PlayerMovementMode::FirstPerson => {
                let norm_dir = camera.direction.normalize();
                let projected_forward = vec3(norm_dir.x, 0.0, norm_dir.z);
                let mut movement_vector = Vector3::zero();
                let control = if let Some(ground) = self.ground { ground.control } else { self.air_control };
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

                    // v: -5, dv: 0.5

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

                if self.ground.is_some() && input.get_key_just_pressed(Key::Named(NamedKey::Space)) {
                    self.velocity.y = self.jump_velocity;
                }
            },
            PlayerMovementMode::FollowCamera => ()
        }
    }
}