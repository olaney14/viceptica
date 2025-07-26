use std::{thread, time::{Duration, Instant}};

use cgmath::{vec3, Matrix4, SquareMatrix, Vector3, Zero};
use glow::{HasContext};
use glutin::surface::GlSurface;
use winit::{event::{DeviceEvent, ElementState, Event, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent}, keyboard::{Key, NamedKey}, platform::modifier_supplement::KeyEventExtModifierSupplement, window::CursorGrabMode};

use crate::{collision::RaycastParameters, common::round_to, mesh::{flags, Mesh}, render::{CameraControlScheme, PointLight}, texture::TextureBank, world::{Model, PlayerMovementMode, Renderable}};

mod ui;
mod mesh;
mod input;
mod world;
mod common;
mod render;
mod shader;
mod window;
mod texture;
mod collision;

const MS_PER_FRAME: u64 = 8;

fn main() {
    let (mut gl, gl_surface, gl_context, window, event_loop) = unsafe { window::create_gl_context() };
    let mut program_bank = shader::ProgramBank::new();
    let mut texture_bank = texture::TextureBank::new();
    let mut mesh_bank = mesh::MeshBank::new();
    let mut input = input::Input::new();
    let mut world = world::World::new();
    // let mut ui = unsafe { ui::UI::new(&mut texture_bank, &gl) };
    let mut ui = ui::implement::VicepticaUI::new(&gl);

    unsafe {
        gl.enable(glow::DEBUG_OUTPUT);
        gl.debug_message_callback(|_, _, _, severity, msg| {
            if severity == glow::DEBUG_SEVERITY_HIGH {
                println!("[OpenGL, high severity] {}", msg);
            } else if severity == glow::DEBUG_SEVERITY_MEDIUM {
                println!("[OpenGL, medium severity] {}", msg);
            }
        });

        ui.init(&mut texture_bank, &mut program_bank, &gl);
        world.scene.load_texture_to_material("test", &mut texture_bank, &gl);
        texture_bank.load_by_name("magic_pixel", &gl).unwrap();
        texture_bank.load_by_name("evil_pixel", &gl).unwrap();
        mesh_bank.add(Mesh::create_square(0.3, 0.2, 0.1, &gl), "square");
        mesh_bank.add(Mesh::create_material_square("test", &gl), "square_textured");
        mesh_bank.add(Mesh::create_material_cube("test", &gl), "cube");
        mesh_bank.add(Mesh::create_cube(&gl), "blank_cube");
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

    let lights = Model::new(
        false,
        Matrix4::from_translation(vec3(-3.0, 0.0, 0.0)),
        vec![
            Renderable::Mesh("blank_cube".to_string(), Matrix4::from_translation(vec3(0.0, 0.0, 0.0)) * Matrix4::from_scale(0.25), flags::FULLBRIGHT),
        ]
    ).with_light(world.scene.add_point_light(PointLight {
        constant: 1.0, linear: 0.14, quadratic: 0.07,
        ambient: vec3(0.15, 0.05, 0.1),
        diffuse: vec3(0.8, 0.3, 0.5),
        specular: vec3(1.0, 1.0, 1.0),
        position: vec3(0.0, 0.0, 0.0)
    }), vec3(0.0, 0.0, 0.0))
    .collider_cuboid(Vector3::zero(), vec3(0.125, 0.125, 0.125));

    unsafe { 
        world.scene.init(&mut texture_bank, &mut mesh_bank, &mut program_bank, &gl);
        world.editor_data.selection_box_vao = Some(mesh::create_selection_cube(&gl));
        world.insert_model(mobile);
        world.set_internal_brushes(brushes);
        world.insert_model(lights);
        world.set_arrows_visible(false);
        world.move_boxes_far();
        world.move_arrows_far();
        world.set_boxes_visible(false);
        world.set_model_visible(world.internal.debug_arrow, false);
    }

    let frame_sleep_duration = Duration::from_millis(MS_PER_FRAME);
    let beginning_of_application = Instant::now();
    let mut elapsed_time = 0.0;
    let mut last_frame = Instant::now();
    let mut cursor_grab_pos = (0, 0);
    let mut grab_cursor = false;

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
                        elapsed_time = (beginning_of_frame - beginning_of_application).as_secs_f64();
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
                                    ui.play_mode = false;
                                },
                                CameraControlScheme::Editor => {
                                    world.scene.camera.control_sceme = CameraControlScheme::FirstPerson(true);
                                    window.set_cursor_grab(CursorGrabMode::Confined).unwrap();
                                    window.set_cursor_visible(false);
                                    world.player.movement = PlayerMovementMode::FirstPerson;
                                    grab_cursor = false;
                                    world.editor_data.active = false;
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
                                if !ui.inner.mouse_captured && input.get_mouse_button_just_pressed(MouseButton::Left) {
                                    world.model_clicked(result);
                                }
                            }
                        } else {
                            if !ui.inner.mouse_captured && input.get_mouse_button_just_pressed(MouseButton::Left) {
                                world.air_clicked();
                            }
                        }
                        
                        world.update(&input, mouse_ray, delta_time);
                        world.scene.camera.update(&input, delta_time);
                        world.scene.update(&mut mesh_bank, &gl);

                        world.scene.render(&mesh_bank, &mut program_bank, &texture_bank, &gl);
                        world.post_render(&mut program_bank, &gl);

                        ui.render_and_update(&input, &mut texture_bank, &mut program_bank, &gl, &mut world);

                        gl_surface.swap_buffers(&gl_context).unwrap();
                        input.update();
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

                                if matches!(world.scene.camera.control_sceme, CameraControlScheme::Editor) {
                                    if *button == MouseButton::Right {
                                        grab_cursor = true;
                                        cursor_grab_pos = (input.mouse_pos.0 as i32, input.mouse_pos.1 as i32);                      
                                    }
                                }
                            },
                            ElementState::Released => {
                                input.on_mouse_button_released(*button);

                                if matches!(world.scene.camera.control_sceme, CameraControlScheme::Editor) {
                                    if *button == MouseButton::Right {
                                        grab_cursor = false;                     
                                    }
                                }
                            }
                        }
                    },
                    WindowEvent::MouseWheel { delta, .. } => {
                        input.set_scroll(
                            match delta {
                                MouseScrollDelta::LineDelta(x, y) => -*y * 40.0,
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
                match event {
                    DeviceEvent::MouseMotion { delta } => {
                        world.scene.camera.mouse_movement(delta.0, -delta.1, &input);
                    },
                    _ => ()
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