use std::{thread, time::{Duration, Instant}};

use cgmath::{vec3, InnerSpace, Matrix4, Rad, SquareMatrix};
use glow::{HasContext};
use glutin::surface::GlSurface;
use winit::{event::{DeviceEvent, ElementState, Event, MouseButton, WindowEvent}, keyboard::{Key, NamedKey}, platform::modifier_supplement::KeyEventExtModifierSupplement, window::CursorGrabMode};

use crate::{mesh::{flags, Mesh}, render::{CameraControlScheme, PointLight}, world::{Model, Renderable}};

mod mesh;
mod input;
mod world;
mod render;
mod shader;
mod window;
mod texture;
mod collision;

const MS_PER_FRAME: u64 = 8;

fn main() {
    let (gl, gl_surface, gl_context, window, event_loop) = unsafe { window::create_gl_context() };
    let mut program_bank = shader::ProgramBank::new();
    let mut texture_bank = texture::TextureBank::new();
    let mut mesh_bank = mesh::MeshBank::new();
    let mut input = input::Input::new();
    let mut world = world::World::new();

    unsafe {
        // texture_bank.load_by_name("test", &gl).unwrap();
        world.scene.load_texture_to_material("test", &mut texture_bank, &gl);
        texture_bank.load_by_name("magic_pixel", &gl).unwrap();
        texture_bank.load_by_name("evil_pixel", &gl).unwrap();
        mesh_bank.add(Mesh::create_square(0.3, 0.2, 0.1, &gl), "square");
        mesh_bank.add(Mesh::create_material_square("test", &gl), "square_textured");
        mesh_bank.add(Mesh::create_material_cube("test", &gl), "cube");
        mesh_bank.add(Mesh::create_cube(&gl), "blank_cube");
    }

    let square_model = Model {
        mobile: false,
        render: vec![
            Renderable::Mesh("square".to_string(), Matrix4::from_translation(vec3(-0.5, 0.0, 0.0)) * Matrix4::from_scale(0.5), 0),
            Renderable::Mesh("square_textured".to_string(), Matrix4::from_translation(vec3(0.5, 0.0, 0.0)) * Matrix4::from_scale(0.5), 0)
        ],
        transform: Matrix4::identity(),
        renderable_indices: Vec::new()
    };

    let mobile = Model {
        mobile: true,
        render: vec![
            Renderable::Mesh("cube".to_string(), Matrix4::from_scale(0.5), 0)
        ],
        transform: Matrix4::identity(),
        renderable_indices: Vec::new()
    };

    // "concrete",
    // "end_sky",
    // "evilwatering",
    // "pillows_old_floor",
    // "sky",
    // "sparkle",
    // "watering"

    let brushes = Model {
        mobile: false,
        render: vec![
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
        ],
        transform: Matrix4::identity(),
        renderable_indices: Vec::new()
    };

    let lights = Model {
        mobile: false,
        render: vec![
            Renderable::Mesh("blank_cube".to_string(), Matrix4::from_translation(vec3(0.0, -7.0, 0.0)) * Matrix4::from_scale(0.25), flags::FULLBRIGHT),
            Renderable::Mesh("blank_cube".to_string(), Matrix4::from_translation(vec3(0.0, 7.0, 0.0)) * Matrix4::from_scale(0.25), flags::FULLBRIGHT),
            Renderable::Mesh("blank_cube".to_string(), Matrix4::from_translation(vec3(0.0, 0.0, 0.0)) * Matrix4::from_scale(0.25), flags::FULLBRIGHT),
            Renderable::Mesh("blank_cube".to_string(), Matrix4::from_translation(vec3(-3.0, 0.0, 0.0)) * Matrix4::from_scale(0.25), flags::FULLBRIGHT)
        ],
        transform: Matrix4::identity(),
        renderable_indices: Vec::new()
    };

    world.scene.add_point_light(PointLight {
        constant: 1.0, linear: 0.7, quadratic: 1.8,
        ambient: vec3(0.1, 0.1, 0.1),
        diffuse: vec3(0.5, 0.5, 0.5),
        specular: vec3(1.0, 1.0, 1.0), 
        position: vec3(0.0, -7.0, 0.0)
    });
    world.scene.add_point_light(PointLight {
        constant: 1.0, linear: 0.35, quadratic: 0.44,
        ambient: vec3(0.1, 0.1, 0.1),
        diffuse: vec3(0.5, 0.5, 0.5),
        specular: vec3(1.0, 1.0, 1.0), 
        position: vec3(0.0, 7.0, 0.0)
    });
    world.scene.add_point_light(PointLight {
        constant: 1.0, linear: 0.14, quadratic: 0.07,
        ambient: vec3(0.15, 0.05, 0.1),
        diffuse: vec3(0.8, 0.3, 0.5),
        specular: vec3(1.0, 1.0, 1.0), 
        position: vec3(0.0, 0.0, 0.0)
    });
    world.scene.add_point_light(PointLight {
        constant: 1.0, linear: 0.35, quadratic: 1.8,
        ambient: vec3(0.1, 0.1, 0.1),
        diffuse: vec3(0.5, 0.0, 0.5),
        specular: vec3(0.0, 1.0, 1.0), 
        position: vec3(-3.0, 0.0, 0.0)
    });

    unsafe { 
        world.scene.init(&mut texture_bank, &mut mesh_bank, &mut program_bank, &gl);
        world.insert_model(square_model);
        world.insert_model(mobile);
        world.insert_model(brushes);
        world.insert_model(lights);
        world.scene.prepare_statics(&mut mesh_bank, &gl);
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

                        if let CameraControlScheme::FirstPerson(locked) = &mut world.scene.camera.control_sceme {
                            if input.get_key_just_pressed(Key::Named(NamedKey::Escape)) && *locked {
                                *locked = false;
                                window.set_cursor_grab(CursorGrabMode::None).unwrap();
                                window.set_cursor_visible(true);
                            }
                        }

                        world.update(&input, delta_time);

                        world.scene.camera.update(&input, delta_time);

                        world.set_model_transform(1, Matrix4::from_axis_angle(vec3(1.0, 1.0, 1.0).normalize(), Rad(elapsed_time as f32)));

                        gl.clear_color(0.0, 0.0, 0.0, 1.0);
                        gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);
                        world.scene.render(&mesh_bank, &mut program_bank, &texture_bank, &gl);

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
    eprintln!("Unimplemented");
}