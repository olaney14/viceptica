use std::{sync::{Mutex, Arc}, thread, time::{Duration, Instant}};

use cgmath::{vec3, Matrix4, SquareMatrix, Vector3, Zero};
use glow::{HasContext};
use glutin::surface::GlSurface;
use winit::{event::{DeviceEvent, ElementState, Event, MouseButton, MouseScrollDelta, WindowEvent}, keyboard::{Key, NamedKey}, platform::modifier_supplement::KeyEventExtModifierSupplement, window::CursorGrabMode};

use crate::{collision::RaycastParameters, component::Component, mesh::flags, render::CameraControlScheme, world::{Model, PlayerMovementMode, Renderable, World}};

mod ui;
mod mesh;
mod save;
mod input;
mod world;
mod common;
mod render;
mod shader;
mod window;
mod texture;
mod collision;
mod component;

const MS_PER_FRAME: u64 = 8;

fn main() {
    let (mut gl, gl_surface, gl_context, window, event_loop) = unsafe { window::create_gl_context() };
    let mut program_bank = shader::ProgramBank::new();
    let mut texture_bank = texture::TextureBank::new();
    let mut mesh_bank = mesh::MeshBank::new();
    let mut input = input::Input::new();
    let mut world = world::World::new();
    let mut ui = ui::implement::VicepticaUI::new(&gl);
    let opengl_debug = Arc::new(Mutex::new(Vec::new()));

    unsafe {
        gl.enable(glow::DEBUG_OUTPUT);
        let debug_clone = opengl_debug.clone();
        gl.debug_message_callback(move |_, _, _, severity, msg| {
            if severity == glow::DEBUG_SEVERITY_HIGH {
                debug_clone.lock().unwrap().push(format!("[OpenGL, high severity] {}", msg));
                println!("[OpenGL, high severity] {}", msg);
            } else if severity == glow::DEBUG_SEVERITY_MEDIUM {
                debug_clone.lock().unwrap().push(format!("[OpenGL, medium severity] {}", msg));
                println!("[OpenGL, medium severity] {}", msg);
            }
        });

        ui.init(&mut texture_bank, &mut program_bank, &gl);
        world.scene.load_texture_to_material("test", &mut texture_bank, &gl);
        texture_bank.load_by_name("magic_pixel", &gl).unwrap();
        texture_bank.load_by_name("evil_pixel", &gl).unwrap();
        World::load_basic_meshes(&mut mesh_bank, &gl);
        world.init(&mut mesh_bank, &gl);
    }

    let mobile = Model::new(
        true,
        Matrix4::identity(),
        vec![
            Renderable::Mesh("cube".to_string(), Matrix4::from_scale(0.5), 0)
        ]
    ).collider_cuboid(Vector3::zero(), vec3(0.25, 0.25, 0.25)).non_solid();

    let brushes = Model::new(
        false,
        Matrix4::identity(),
        vec![
            Renderable::Brush("ice".to_string(), vec3(0.0, -5.0, 0.0), vec3(20.0, 1.0, 20.0), flags::EXTEND_TEXTURE),
            Renderable::Brush("concrete".to_string(), vec3(0.0, -4.0, 0.0), vec3(8.0, 1.0, 8.0), flags::EXTEND_TEXTURE),
            Renderable::Brush("pillows_old_floor".to_string(), vec3(5.0, 0.0, 0.0), vec3(1.0, 4.0, 4.0), flags::EXTEND_TEXTURE),
            Renderable::Brush("end_sky".to_string(), vec3(0.0, 5.0, 0.0), vec3(2.0, 2.0, 2.0), flags::EXTEND_TEXTURE),
            Renderable::Brush("evilwatering".to_string(), vec3(3.0, 0.0, 0.0), vec3(2.0, 2.0, 2.0), flags::EXTEND_TEXTURE),
            Renderable::Brush("container".to_string(), vec3(-5.0, 0.0, 0.0), vec3(1.0, 10.0, 10.0), flags::EXTEND_TEXTURE),
            Renderable::Brush("sky".to_string(), vec3(2.0, 0.0, 0.0), vec3(1.0, 7.0, 7.0), flags::EXTEND_TEXTURE),
            Renderable::Brush("concrete".to_string(), vec3(1.0, -2.0, 0.0), vec3(1.0, 2.0, 1.0), flags::EXTEND_TEXTURE),
            Renderable::Brush("tar".to_string(), vec3(0.0, -5.0, 15.0), vec3(10.0, 1.0, 10.0), flags::EXTEND_TEXTURE),
            Renderable::Brush("watering".to_string(), vec3(0.0, -4.5, 0.0), vec3(10.0, 1.0, 10.0), flags::EXTEND_TEXTURE)
        ]
    );

    unsafe { texture_bank.load_by_name("komari", &gl).unwrap(); }
    let billboard = Model::new(
        true,
        Matrix4::from_translation(vec3(0.0, 1.0, 0.0)) * Matrix4::from_nonuniform_scale(1.0, 2.0, 1.0),
        vec![
            Renderable::Billboard("komari".to_string(), vec3(0.0, 0.0, 0.0), (1.0, 2.0), flags::FULLBRIGHT | flags::CUTOUT, false)
        ]
    ).collider_cuboid(vec3(0.0, 0.0, 0.0), vec3(0.125, 0.125, 0.125)).non_solid();

    let door = Model::new(
        true,
        Matrix4::from_translation(vec3(0.0, -2.0, 0.0)),
        vec![
            Renderable::Brush("container".to_string(), vec3(0.0, 0.0, 0.0), vec3(2.0, 4.0, 0.25), flags::EXTEND_TEXTURE)
        ]
    ).with_component(Component::Door(component::Door::new(8.0, 3.75, 200)));

    unsafe { 
        world.scene.init(&mut texture_bank, &mut mesh_bank, &mut program_bank, &gl);
        world.editor_data.selection_box_vao = Some(mesh::create_selection_cube(&gl));
        world.insert_model(mobile);
        world.insert_model(billboard);
        world.insert_model(door);
        world.set_internal_brushes(brushes);
        // world.insert_model(lights);
        world.set_arrows_visible(false);
        world.move_boxes_far();
        world.move_arrows_far();
        world.set_boxes_visible(false);
        world.set_model_visible(world.internal.debug_arrow, false);
    }

    let frame_sleep_duration = Duration::from_millis(MS_PER_FRAME);
    let mut last_frame = Instant::now();
    let mut cursor_grab_pos = (0, 0);
    let mut grab_cursor = false;

    let mut drawing_box = false;
    let mut box_origin = (0, 0);
    let mut selection_box_valid = false;

    // https://github.com/grovesNL/glow/blob/main/examples/hello/src/main.rs
    let _ = event_loop.run(move |event, elwt| {
        match event {
            Event::WindowEvent { ref event, .. } => {
                match event {
                    WindowEvent::CloseRequested => {
                        elwt.exit();
                    },
                    WindowEvent::RedrawRequested => unsafe {
                        let beginning_of_frame = Instant::now();
                        let delta_time = (beginning_of_frame - last_frame).as_secs_f32();
                        last_frame = beginning_of_frame;

                        if input.get_key_pressed(Key::Named(NamedKey::Control)) && input.get_key_just_pressed(Key::Character("e".into())) {
                            match world.scene.camera.control_sceme {
                                CameraControlScheme::FirstPerson(..) => {
                                    world.scene.camera.control_sceme = CameraControlScheme::Editor;
                                    window.set_cursor_grab(CursorGrabMode::None).unwrap();
                                    window.set_cursor_visible(true);
                                    world.player.movement = PlayerMovementMode::FollowCamera;
                                    grab_cursor = false;
                                    world.editor_data.active = true;
                                    world.do_game_logic = false;
                                    ui.play_mode = false;
                                },
                                CameraControlScheme::Editor => {
                                    world.scene.camera.control_sceme = CameraControlScheme::FirstPerson(true);
                                    window.set_cursor_grab(CursorGrabMode::Confined).unwrap();
                                    window.set_cursor_visible(false);
                                    world.player.movement = PlayerMovementMode::FirstPerson;
                                    grab_cursor = false;
                                    world.editor_data.active = false;
                                    world.do_game_logic = true;
                                    world.deselect();
                                    ui.play_mode = true;
                                }
                            }
                        }

                        if input.get_key_pressed(Key::Named(NamedKey::Control)) && input.get_key_just_pressed(Key::Character("m".into())) {
                            println!("{}", mesh_bank.log_loaded_models());
                        }

                        if input.get_key_pressed(Key::Named(NamedKey::Control)) && input.get_key_just_pressed(Key::Character("b".into())) {
                            world.debug_brushes();
                        }

                        if let CameraControlScheme::FirstPerson(locked) = &mut world.scene.camera.control_sceme {
                            if input.get_key_just_pressed(Key::Named(NamedKey::Escape)) && *locked {
                                *locked = false;
                                window.set_cursor_grab(CursorGrabMode::None).unwrap();
                                window.set_cursor_visible(true);
                            }
                        }

                        let mouse_ray = world.get_mouse_ray(input.mouse_pos.0, input.mouse_pos.1, window.inner_size().width, window.inner_size().height);
                        if let Some(result) = world.physical_scene.raycast(mouse_ray.0, mouse_ray.1, 100.0, &RaycastParameters::new().ignore(vec![world.player.collider]).select_foreground()) {
                            if result.model.is_some() {
                                if !ui.inner.mouse_captured {
                                    let shift_pressed = input.get_key_pressed(Key::Named(NamedKey::Shift));
                                    if input.get_mouse_button_just_released(MouseButton::Left) && !selection_box_valid && world.editor_data.drag_axis.is_none() {
                                        world.model_released(result, shift_pressed);
                                    } else if input.get_mouse_button_just_pressed(MouseButton::Left) {
                                        world.model_pressed(result);
                                    }
                                }
                            }
                        } else {
                            if !ui.inner.mouse_captured && input.get_mouse_button_just_released(MouseButton::Left) && world.editor_data.drag_axis.is_none() && !selection_box_valid {
                                world.air_clicked();
                            }
                        }
                        if !ui.inner.mouse_captured && world.editor_data.active && input.get_mouse_button_just_pressed(MouseButton::Left) && world.editor_data.drag_axis.is_none() {
                            drawing_box = true;
                            box_origin = (input.mouse_pos.0 as i32, input.mouse_pos.1 as i32);
                        }

                        if input.get_mouse_button_released(MouseButton::Left) {
                            drawing_box = false;
                        }

                        if drawing_box {
                            let mouse_pos = (input.mouse_pos.0 as i32, input.mouse_pos.1 as i32);
                            // let area = ((box_origin.0 - mouse_pos.0) * (box_origin.1 - mouse_pos.1)).abs();

                            let x = box_origin.0.min(mouse_pos.0);
                            let y = box_origin.1.min(mouse_pos.1);
                            let w = (box_origin.0 - mouse_pos.0).abs() as u32;
                            let h = (box_origin.1 - mouse_pos.1).abs() as u32;

                            selection_box_valid = false;
                            if w > 32 && h > 32 {
                                selection_box_valid = true;
                                ui.selection_box(x, y, w, h);

                                if input.get_mouse_button_just_released(MouseButton::Left) {
                                    drawing_box = false;
                                    if world.editor_data.active {
                                        let screen_size = window.inner_size();
                                        let models = world.get_models_within_rect(box_origin.0, box_origin.1, mouse_pos.0, mouse_pos.1, screen_size.width, screen_size.height);
                                        if models.len() == 1 {
                                            world.select_model(models[0]);
                                            world.set_arrows_visible(true);
                                        } else if models.len() > 1 {
                                            if !models.is_empty() {
                                                world.deselect();
                                                for model in models.iter() {
                                                    world.select_or_append_model(*model);
                                                }
                                            }
                                        }

                                        let brushes = world.get_brushes_within_rect(box_origin.0, box_origin.1, mouse_pos.0, mouse_pos.1, screen_size.width, screen_size.height);
                                        if models.is_empty() && brushes.len() == 1 {
                                            world.set_arrows_visible(true);
                                            world.select_brush(brushes[0]);
                                        } else if !brushes.is_empty() {
                                            if models.is_empty() {
                                                world.deselect();
                                            }
                                            for brush in brushes.iter() {
                                                world.select_or_append_brush(*brush);
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        
                        if world.editor_data.active && !ui.inner.mouse_captured && input.scroll.abs() > 0.01 {
                            world.scene.camera.pos -= world.scene.camera.direction * 0.05 * input.scroll;
                        }

                        world.update(&input, mouse_ray, delta_time);
                        world.scene.camera.update(&input, delta_time);
                        world.scene.update(&mut mesh_bank, &gl);

                        world.scene.render(&mesh_bank, &mut program_bank, &texture_bank, &gl);
                        world.post_render(&mut program_bank, &gl);

                        for line in world.editor_data.show_debug.drain(..) {
                            ui.show_debug(&line);
                        }
                        for line in opengl_debug.lock().unwrap().drain(..) {
                            ui.show_debug(&line);
                        }
                        ui.render_and_update(&input, &mut texture_bank, &mut program_bank, &gl, &mut world);

                        gl_surface.swap_buffers(&gl_context).unwrap();

                        input.update();
                        if let Some(level_data) = world.load_new.take() {
                            let mut new_world = World::from_save_data(level_data, &mut texture_bank, &mut mesh_bank, &mut program_bank, &gl);
                            new_world.scene.camera.control_sceme = world.scene.camera.control_sceme.clone();
                            new_world.player.movement = world.player.movement.clone();
                            new_world.editor_data.active = world.editor_data.active;
                            new_world.editor_data.increment = world.editor_data.increment;
                            new_world.editor_data.save_to = world.editor_data.save_to.clone();
                            let window_size =  window.inner_size(); 
                            new_world.scene.camera.on_window_resized(window_size.width as f32, window_size.height as f32);
                            world = new_world;
                        }

                        let frame_duration = Instant::now() - beginning_of_frame;
                        if let Some(duration) = frame_sleep_duration.checked_sub(frame_duration) {
                            thread::sleep(duration);
                        } 
                        window.request_redraw();
                    },
                    WindowEvent::KeyboardInput { event, .. } => {
                        match event.state {
                            ElementState::Pressed => {
                                input.on_key_pressed(event.key_without_modifiers());
                            },
                            ElementState::Released => {
                                input.on_key_released(event.key_without_modifiers());
                            }
                        }
                    },
                    WindowEvent::MouseInput { state, button, .. } => {
                        match state {
                            ElementState::Pressed => {
                                input.on_mouse_button_pressed(*button);
                                if let CameraControlScheme::FirstPerson(locked) = &mut world.scene.camera.control_sceme {
                                    if !*locked {
                                        *locked = true;
                                        window.set_cursor_grab(CursorGrabMode::Confined).unwrap();
                                        window.set_cursor_visible(false);
                                    }
                                }

                                if matches!(world.scene.camera.control_sceme, CameraControlScheme::Editor) && *button == MouseButton::Right {
                                    grab_cursor = true;
                                    cursor_grab_pos = (input.mouse_pos.0 as i32, input.mouse_pos.1 as i32);                      
                                }
                            },
                            ElementState::Released => {
                                input.on_mouse_button_released(*button);

                                if matches!(world.scene.camera.control_sceme, CameraControlScheme::Editor) && *button == MouseButton::Right {
                                    grab_cursor = false;                     
                                }
                            }
                        }
                    },
                    WindowEvent::MouseWheel { delta, .. } => {
                        input.set_scroll(
                            match delta {
                                MouseScrollDelta::LineDelta(_, y) => -*y * 40.0,
                                MouseScrollDelta::PixelDelta(px) => px.y as f32
                            }
                        );
                    },
                    WindowEvent::CursorMoved { position, .. } => {
                        input.on_mouse_moved(position.x, position.y);

                        if grab_cursor {
                            let window_pos = window.inner_position().unwrap();
                            set_mouse_pos(cursor_grab_pos.0 + window_pos.x, cursor_grab_pos.1 + window_pos.y).unwrap();
                        }
                    },
                    WindowEvent::Resized(new_size) => unsafe {
                        gl.viewport(0, 0, new_size.width as i32, new_size.height as i32);
                        world.scene.camera.on_window_resized(new_size.width as f32, new_size.height as f32);
                        ui.inner.screen_size = (new_size.width, new_size.height);
                    },
                    _ => ()
                }
            },
            Event::DeviceEvent { ref event, .. } => {
                if let DeviceEvent::MouseMotion { delta } = event {
                    world.scene.camera.mouse_movement(delta.0, -delta.1, &input);
                }
            }
            _ => ()
        }
    });
}

#[cfg(target_os = "windows")]
fn set_mouse_pos(x: i32, y: i32) -> Result<(), String> {
    unsafe {
        if winapi::um::winuser::SetCursorPos(x, y) == 0 {
            Err("SetCursorPos failed".into())
        } else {
            Ok(())
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn set_mouse_pos(x: i32, y: i32) -> Result<(), String> {
    todo!();
}