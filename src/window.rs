use std::num::NonZeroU32;

use glutin::{config::{ConfigTemplateBuilder, GlConfig}, context::{ContextApi, ContextAttributesBuilder, GlProfile}, display::GetGlDisplay, prelude::{GlDisplay, NotCurrentGlContext}, surface::{GlSurface, SwapInterval, WindowSurface}};
use glutin_winit::{DisplayBuilder, GlWindow};
use raw_window_handle::HasRawWindowHandle;
use winit::event_loop::EventLoop;

pub type ProgramContext = (glow::Context, glutin::surface::Surface<WindowSurface>, glutin::context::PossiblyCurrentContext, winit::window::Window, EventLoop<()>);

// https://github.com/grovesNL/glow/blob/main/examples/hello/src/main.rs
pub unsafe fn create_gl_context() -> ProgramContext {
    let event_loop = winit::event_loop::EventLoopBuilder::new().build().unwrap();
    let window_builder = winit::window::WindowBuilder::new()
        .with_title("VICEPTICA")
        .with_inner_size(winit::dpi::LogicalSize::new(640.0 * 2.0, 480.0 * 2.0));

    let template = ConfigTemplateBuilder::new()
        .with_stencil_size(8);

    let display_builder = DisplayBuilder::new().with_window_builder(Some(window_builder));

    // choose the config with the biggest multisample buffer
    let (window, gl_config) = display_builder
        .build(&event_loop, template, |configs| {
            configs.reduce(|accum, config| {
                if config.num_samples() > accum.num_samples() {
                    config
                } else {
                    accum
                }
            }).unwrap()
        }).unwrap();

    let raw_window_handle = window.as_ref().map(|window| window.raw_window_handle());

    let gl_display = gl_config.display();
    
    // gl version 4.1 
    let context_attributes = ContextAttributesBuilder::new()
        .with_context_api(ContextApi::OpenGl(Some(glutin::context::Version {
            major: 4,
            minor: 1
        })))
        .with_profile(GlProfile::Core)
        .build(raw_window_handle);

    let not_current_gl_context = gl_display
            .create_context(&gl_config, &context_attributes)
            .unwrap();

    let window = window.unwrap();

    let attrs = window.build_surface_attributes(Default::default());
    let gl_surface = gl_display
            .create_window_surface(&gl_config, &attrs)
            .unwrap();

    let gl_context = not_current_gl_context.make_current(&gl_surface).unwrap();

    let gl = glow::Context::from_loader_function_cstr(|s| gl_display.get_proc_address(s));

    gl_surface
        .set_swap_interval(&gl_context, SwapInterval::Wait(NonZeroU32::new(1).unwrap()))
        .unwrap();

    (
        gl,
        gl_surface,
        gl_context,
        window,
        event_loop
    )
}