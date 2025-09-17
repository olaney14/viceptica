use std::mem;

use cgmath::{vec3, EuclideanSpace, Matrix4, MetricSpace, Point3, Transform, Vector3};
use serde::{Deserialize, Serialize};

use crate::{common, effects::{FogEffect, KernelEffect}, world::{Model, Renderable, World}};

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
pub struct Trigger {
    pub kind: TriggerType,
    #[serde(skip)]
    pub player_within: bool,
    #[serde(skip)]
    pub invalid: bool
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum TriggerType {
    SetFogEffect { 
        enabled: bool, color: [f32; 3], strength: f32, max: f32, 
        #[serde(skip)]
        max_tween: f32
    },
    SetKernelEffect { enabled: bool, kernel: [f32; 9], offset: f32 },
    Test { enter: String, update: String, exit: String }
}

impl Trigger {
    pub fn new(kind: TriggerType) -> Self {
        Self {
            invalid: true,
            player_within: false,
            kind
        }
    }

    pub fn on_enter(component: &mut Component, model: &mut Model, world: &mut World) {
        if let Component::Trigger(trigger) = component {
            match &mut trigger.kind {
                TriggerType::Test { enter, .. } => println!("{}", enter),
                TriggerType::SetFogEffect { enabled, color, strength, max , max_tween} => {
                    // if *enabled {
                    //     world.scene.post_process.fog = Some(FogEffect {
                    //         max: *max,
                    //         strength: *strength,
                    //         color: vec3(color[0], color[1], color[2])
                    //     });
                    // } else {
                    //     world.scene.post_process.fog = None;
                    // }
                    if *enabled {
                        *max_tween = world.scene.post_process.fog.as_ref().map_or(0.0, |f| f.max);
                        world.scene.post_process.fog = Some(FogEffect {
                            max: *max_tween,
                            strength: *strength,
                            color: vec3(color[0], color[1], color[2])
                        });
                    }
                },
                TriggerType::SetKernelEffect { enabled, kernel, offset } => {
                    if *enabled {
                        world.scene.post_process.kernel = Some(KernelEffect {
                            kernel: *kernel,
                            offset: *offset
                        })
                    }
                }
            }
        }
    }

    pub fn update_inside(component: &mut Component, model: &mut Model, world: &mut World) {
        if let Component::Trigger(trigger) = component {
            match &mut trigger.kind {
                TriggerType::Test { update, .. } => println!("{}", update),
                TriggerType::SetFogEffect { enabled, color, strength, max, max_tween } => {
                    if *enabled {
                        if !common::fuzzy_eq(*max, *max_tween, 0.011) {
                            *max_tween += common::towards(*max_tween, *max, 0.01);

                            if common::fuzzy_eq(*max, *max_tween, 0.011) {
                                *max_tween = *max;
                            }

                            world.scene.post_process.fog = Some(FogEffect {
                                max: *max_tween,
                                strength: *strength,
                                color: vec3(color[0], color[1], color[2])
                            });
                        }
                    }
                }
                _ => ()
            }
        }
    }

    pub fn on_exit(component: &mut Component, model: &mut Model, world: &mut World) {
        if let Component::Trigger(trigger) = component {
            match &mut trigger.kind {
                TriggerType::Test { exit, .. } => println!("{}", exit),
                TriggerType::SetKernelEffect { .. } => {
                    world.scene.post_process.kernel = world.scene.world_default_effects.kernel.clone();
                },
                _ => ()
            }
        }
    }

    pub fn update_outside(component: &mut Component, model: &mut Model, world: &mut World) {
        if let Component::Trigger(trigger) = component {
            match &mut trigger.kind {
                TriggerType::SetFogEffect { enabled, color, strength, max, max_tween } => {
                    if *enabled {
                        let world_target = world.scene.world_default_effects.fog.as_ref().map_or(0.0, |f| f.max);
                        if !common::fuzzy_eq(*max_tween, world_target, 0.011) {
                            *max_tween += common::towards(*max_tween, world_target, 0.01);

                            if common::fuzzy_eq(*max, world_target, 0.011) {
                                world.scene.post_process.fog = world.scene.world_default_effects.fog.clone();
                            } else {
                                world.scene.post_process.fog = Some(FogEffect {
                                    max: *max_tween,
                                    strength: *strength,
                                    color: vec3(color[0], color[1], color[2])
                                });
                            }
                        }
                    }
                },
                _ => ()
            }
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
    Dummy,
    /// Behavior on entry, exit<br>
    /// Trigger is expected to be placed on a model with a single brush inside
    Trigger(Trigger)
}

impl Component {
    /// Called before the model is put into the scene
    pub fn on_insert(this: usize, model: &mut Model, world: &mut World) {
        match &mut model.components[this] {
            Component::Door(_) => {
                if !model.mobile {
                    model.mobile = true;
                    world.editor_data.show_debug.push(String::from("made model mobile because it had a Door component"));
                }
            },
            Component::Trigger(trigger) => {
                if model.render.len() != 1 {
                    world.editor_data.show_debug.push(String::from("Expected only one element"));
                    trigger.invalid = true;
                } else if !matches!(model.render[0], Renderable::Brush(..)) {
                    world.editor_data.show_debug.push(String::from("Singular element in trigger model was not brush"));
                    trigger.invalid = true;
                }
                trigger.invalid = false;
                trigger.player_within = false;
            }
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
            },
            Component::Trigger(trigger) => {
                // this was checked on insert
                let (mut brush_origin, mut brush_extents) = 
                    if let Renderable::Brush(_, origin, extents, _) = model.render[0] { 
                        (origin, extents) 
                    } else {
                        panic!("First (supposedly only) element in trigger model was not a brush");
                    };
                brush_origin += common::translation(model.transform);
                brush_extents = model.transform.transform_vector(brush_extents);

                let min = (brush_origin - brush_extents / 2.0);
                let max = (brush_origin + brush_extents / 2.0);

                let within_brush = {
                    let pp = &world.scene.camera.pos;
                    pp.x > min.x && pp.y > min.y && pp.z > min.z && pp.x < max.x && pp.y < max.y && pp.z < max.z
                };

                if trigger.player_within {
                    if !within_brush {
                        trigger.player_within = false;
                        Trigger::on_exit(&mut component, &mut model, world);
                    } else {
                        Trigger::update_inside(&mut component, &mut model, world);
                    }
                } else {
                    if within_brush {
                        trigger.player_within = true;
                        Trigger::on_enter(&mut component, &mut model, world);
                    } else {
                        Trigger::update_outside(&mut component, &mut model, world);
                    }
                }
            }
            _ => ()
        }

        mem::swap(&mut component, &mut model.components[this]);

        model
    }

    // on_remove
}