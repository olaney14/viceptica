use std::{thread, time::{Duration, Instant}};

use cgmath::{vec3, InnerSpace, Matrix4, Rad, SquareMatrix};
use glow::{HasContext};
use glutin::surface::GlSurface;
use winit::{event::{ElementState, Event, WindowEvent}, keyboard::Key, platform::modifier_supplement::KeyEventExtModifierSupplement};

use crate::{mesh::Mesh, world::{Model, Renderable}};

mod mesh;
mod input;
mod world;
mod render;
mod shader;
mod window;
mod texture;

const MS_PER_FRAME: u64 = 16;

fn main() {
    let (gl, gl_surface, gl_context, window, event_loop) = unsafe { window::create_gl_context() };
    let mut program_bank = shader::ProgramBank::new();
    let mut texture_bank = texture::TextureBank::new();
    let mut mesh_bank = mesh::MeshBank::new();
    let mut input = input::Input::new();

    unsafe {
        texture_bank.load_by_name("test", &gl).unwrap();
        texture_bank.load_by_name("magic_pixel", &gl).unwrap();
        mesh_bank.add(Mesh::create_square(0.3, 0.2, 0.1, &gl), "square");
        mesh_bank.add(Mesh::create_textured_square("test", &gl), "square_textured");
        mesh_bank.add(Mesh::create_textured_cube("test", &gl), "cube");
    }

    let square_model = Model {
        mobile: false,
        render: vec![
            Renderable::Mesh("square".to_string(), Matrix4::from_translation(vec3(-0.5, 0.0, 0.0)) * Matrix4::from_scale(0.5)),
            Renderable::Mesh("square_textured".to_string(), Matrix4::from_translation(vec3(0.5, 0.0, 0.0)) * Matrix4::from_scale(0.5))
        ],
        transform: Matrix4::identity(),
        renderable_indices: Vec::new()
    };

    let mobile = Model {
        mobile: true,
        render: vec![
            Renderable::Mesh("cube".to_string(), Matrix4::from_scale(0.5))
        ],
        transform: Matrix4::identity(),
        renderable_indices: Vec::new()
    };

    let mut world = world::World::new();
    unsafe { 
        world.insert_model(square_model);
        world.insert_model(mobile);
        world.scene.init(&mut program_bank, &gl);
        world.scene.prepare_statics(&mut mesh_bank, &gl);
    }

    let frame_sleep_duration = Duration::from_millis(MS_PER_FRAME);
    let beginning_of_application = Instant::now();
    let mut elapsed_time = 0.0;
    let mut last_frame = Instant::now();

    // https://github.com/grovesNL/glow/blob/main/examples/hello/src/main.rs
    let _ = event_loop.run(move |event, elwt| {
        if let Event::WindowEvent { event, .. } = event {
            match event {
                WindowEvent::CloseRequested => {
                    elwt.exit();
                },
                WindowEvent::RedrawRequested => unsafe {
                    let beginning_of_frame = Instant::now();
                    elapsed_time = (beginning_of_frame - beginning_of_application).as_secs_f64();
                    let delta_time = (beginning_of_frame - last_frame).as_secs_f32();
                    last_frame = beginning_of_frame;

                    world.scene.camera.update(&input, delta_time);

                    world.set_model_transform(1, Matrix4::from_axis_angle(vec3(1.0, 1.0, 1.0).normalize(), Rad(elapsed_time as f32)));

                    gl.clear_color(0.1, 0.2, 0.3, 1.0);
                    gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);
                    world.scene.render(&mesh_bank, &mut program_bank, &texture_bank, &gl);
                    window.request_redraw();

                    gl_surface.swap_buffers(&gl_context).unwrap();

                    let frame_duration = Instant::now() - beginning_of_frame;
                    if let Some(duration) = frame_sleep_duration.checked_sub(frame_duration) {
                        thread::sleep(duration);
                    }
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
                WindowEvent::Resized(new_size) => unsafe {
                    gl.viewport(0, 0, new_size.width as i32, new_size.height as i32);
                    world.scene.camera.on_window_resized(new_size.width as f32, new_size.height as f32);
                },
                _ => ()
            }
        }
    });
}