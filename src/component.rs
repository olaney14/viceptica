use std::mem;

use cgmath::{vec3, EuclideanSpace, Matrix4, MetricSpace, Point3, Vector3};
use serde::{Deserialize, Serialize};

use crate::{common, world::{Model, World}};

fn zero_vec_slice() -> [f32; 3] {
    [0.0; 3]
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Door {
    pub radius: f32,
    pub height: f32,
    pub open_time: u32,
    #[serde(skip, default="zero_vec_slice")]
    origin: [f32; 3],
    #[serde(skip)]
    open_progress: u32,
    #[serde(skip)]
    opened: bool
}

impl Door {
    pub fn new(radius: f32, height: f32, open_time: u32) -> Self {
        Self {
            radius, height, opened: false,
            open_time, origin: [0.0; 3],
            open_progress: 0
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum Component {
    /// Marker for spawning the player
    Spawnpoint,
    /// goes up when the player near it
    Door(Door),
    /// The Rust Programming Language
    Dummy
}

impl Component {
    /// Called before the model is put into the scene
    pub fn on_insert(this: usize, model: &mut Model, world: &mut World) {
        match model.components[this] {
            Component::Door(_) => {
                if !model.mobile {
                    model.mobile = true;
                    world.editor_data.show_debug.push(String::from("made model mobile because it had a Door component"));
                }
            },
            _ => ()
        }
    }

    /// Called on each update loop
    pub fn on_update(this: usize, mut model: Model, world: &mut World) -> Model {
        let mut component = mem::replace(&mut model.components[this], Component::Dummy);

        match &mut component {
            Component::Door(door) => {
                if !door.opened {
                    door.origin = model.origin().into();
                }
                if world.do_game_logic {
                    let origin: Vector3<f32> = door.origin.into();
                    let dist2 = world.scene.camera.pos.distance2(Point3::from_vec(origin));
                    if dist2 < door.radius.powf(2.0) {
                        if door.open_progress < door.open_time {
                            door.open_progress += 1;
                        }
                        if !door.opened {
                            door.opened = true;
                        }
                    } else {
                        if door.open_progress > 0 {
                            door.open_progress -= 1;
                        } else {
                            door.opened = false;
                            let original_transform = Matrix4::from_translation(origin) * common::mat4_remove_translation(model.transform);
                            model = world.set_model_transform_external(model, original_transform);
                        }
                    }

                    if door.open_progress > 0 {
                        let original_transform = Matrix4::from_translation(origin) * common::mat4_remove_translation(model.transform);
                        let new_transform = Matrix4::from_translation(vec3(0.0, (door.height / door.open_time as f32) * door.open_progress as f32, 0.0)) * original_transform;
                        model = world.set_model_transform_external(model, new_transform);
                    }
                } else {
                    if door.opened {
                        door.opened = false;
                        door.open_progress = 0;
                        let original_transform = Matrix4::from_translation(door.origin.into()) * common::mat4_remove_translation(model.transform);
                        model = world.set_model_transform_external(model, original_transform);
                    }
                }
            },
            Component::Dummy => {
                world.editor_data.show_debug.push(String::from("Dummy component found in model"));
            }
            _ => ()
        }

        mem::swap(&mut component, &mut model.components[this]);

        model
    }

    // on_remove
}