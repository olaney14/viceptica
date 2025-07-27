use std::{cell::RefCell, rc::Rc};

use cgmath::vec2;
use glow::{HasContext, NativeVertexArray};
use winit::event::MouseButton;

use crate::{input::Input, shader::{Program, ProgramBank}, texture::TextureBank, ui};

const FONT_CHARS: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789 .!,- ?  _";
const FONT_WIDTH: usize = 10;
// const FONT_HEIGHT: usize = 8;

#[derive(Debug)]
enum FrameType {
    Simple,
    Interactable
}

pub enum FrameInteraction {
    Close,
    DragBegin,
    ResizeBegin,
    Scroll(f32),
    OtherContentsClicked
}

pub struct SliderInteraction {
    pub clicked: bool,
    pub progress: u32
}

impl FrameType {
    fn get_texture_origin(&self) -> (u32, u32) {
        match self {
            Self::Simple => (0, 0),
            Self::Interactable => (48, 0)
        }
    }
}

#[derive(Debug)]
struct NineCell {
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    frame_type: FrameType
}

#[derive(Debug)]
struct TextLabel {
    x: i32,
    y: i32,
    message: String
}

#[derive(Debug)]
struct TextureLabel {
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    tx: u32, ty: u32, tw: u32, th: u32,
    texture: String
}

#[derive(Debug)]
struct Slider {
    x: i32, y: i32,
    size: u32, slider_pos: u32,
    vertical: bool
}

#[derive(Debug)]
enum ElementType {
    NineCell(NineCell),
    TextLabel(TextLabel),
    TextureLabel(TextureLabel),
    TitledNineCell(NineCell, String),
    Slider(Slider),
    None
}

type NodePtr = Rc<RefCell<UINode>>;

fn node_ptr(node: UINode) -> NodePtr {
    Rc::new(RefCell::new(node))
}

struct UINode {
    children: Vec<NodePtr>,
    clip: (i32, i32, u32, u32),
    draw: ElementType,
    focus: u32,
    x: i32,
    y: i32
}

impl UINode {
    pub fn root() -> Self {
        Self { children: Vec::new(), clip: (0, 0, 640 * 2, 480 * 2), draw: ElementType::None, focus: 0, x: 0, y: 0 }
    }

    /// Recursively sort all UI nodes
    pub fn sort_all(&mut self) {
        self.children.sort_by(|a, b| a.borrow().focus.cmp(&b.borrow().focus));

        for child in self.children.iter_mut() {
            child.borrow_mut().sort_all();
        }
    }
}

pub struct UI {
    tree: NodePtr,
    current_node: NodePtr,
    parent_nodes: Vec<NodePtr>,
    last_modified: NodePtr,
    /// Dummy vao, the UI doesn't use any vertex data
    pub vao: NativeVertexArray,
    pub screen_size: (u32, u32),
    pub mouse_captured: bool,
    pub inc_focus: u32,
    current_global_origin: (i32, i32)
}

impl UI {
    pub unsafe fn new(gl: &glow::Context) -> Self {
        let vao = gl.create_vertex_array().unwrap();
        let tree = Rc::new(RefCell::new(UINode::root()));
        Self {
            vao,
            tree: tree.clone(),
            current_node: tree.clone(),
            parent_nodes: Vec::new(),
            screen_size: (640 * 2, 480 * 2),
            mouse_captured: false,
            inc_focus: 0,
            last_modified: tree.clone(),
            current_global_origin: (0, 0)
        }
    }

    pub fn begin(&mut self) {
        self.tree = Rc::new(RefCell::new(UINode::root()));
        self.current_node = self.tree.clone();
        self.last_modified = self.tree.clone();
        self.parent_nodes.clear();
        self.mouse_captured = false;
        self.current_global_origin = (0, 0);
    }

    fn add_child(&mut self, element: UINode) {
        let element = node_ptr(element);
        self.current_node.as_ref().borrow_mut().children.push(element.clone());
        self.last_modified = element;
    }

    fn add_child_as_current(&mut self, element: UINode) {
        self.current_global_origin.0 += element.x;
        self.current_global_origin.1 += element.y;
        let element = node_ptr(element);
        self.current_node.as_ref().borrow_mut().children.push(element.clone());
        self.parent_nodes.push(self.current_node.clone());
        self.current_node = element.clone();
        self.last_modified = element;
    }

    fn _frame(&mut self, x: i32, y: i32, w: u32, h: u32, frame: FrameType, title: &str) {
        if title.len() == 0 {
            self.add_child_as_current(UINode {
                children: Vec::new(),
                clip: (1, 17, w - 2, h - 18),
                draw: ElementType::NineCell(NineCell {
                    x: 0, y: 0, w, h, frame_type: frame
                }),
                focus: self.inc_focus, x, y
            });
        } else {
            self.add_child_as_current(UINode {
                children: Vec::new(),
                clip: (1, 17, w - 2, h - 18),
                draw: ElementType::TitledNineCell(NineCell {
                    x: 0, y: 0, w, h, frame_type: frame
                }, title.to_string()),
                focus: self.inc_focus, x, y
            });
        }

        self.inc_focus += 1;
    }

    pub fn frame(&mut self, x: i32, y: i32, w: u32, h: u32) {
        self._frame(x, y, w, h, FrameType::Simple, "");
    }

    pub fn set_focus(&mut self, focus: u32) {
        self.last_modified.borrow_mut().focus = focus;
    }

    fn global_clip_rect(&self) -> (i32, i32, u32, u32) {
        let mut clip = self.current_node.borrow().clip.clone();
        clip.0 += self.current_global_origin.0;
        clip.1 += self.current_global_origin.1;
        clip
    }

    pub fn mouse_in_clip_rect(&self, mpx: i32, mpy: i32) -> bool {
        let clip = self.global_clip_rect();
        mpx > clip.0 && mpx < clip.0 + clip.2 as i32 && mpy > clip.1 && mpy < clip.1 + clip.3 as i32
    }

    pub fn mouse_in_frame(&self, mpx: i32, mpy: i32) -> bool {
        let clip = self.global_clip_rect();
        mpx > clip.0 && mpx < clip.0 + clip.2 as i32 && mpy > clip.1 - 16 && mpy < clip.1 + clip.3 as i32 + 16
    }

    pub fn interactable_frame(&mut self, input: &Input, title: &str, x: i32, y: i32, w: u32, h: u32) -> Option<FrameInteraction> {
        let mpx = input.mouse_pos.0 as i32;
        let mpy = input.mouse_pos.1 as i32;
        let gx = x + self.current_global_origin.0;
        let gy = y + self.current_global_origin.1;
        let mouse_within_x = mpx > gx + w as i32 - 16 && mpx < gx + w as i32 && mpy > gy && mpy < gy + 16;
        let mouse_within_bar = mpx > gx as i32 && mpx < gx + w as i32 && mpy > gy && mpy < gy + 16;
        let mouse_within_body = mpx > gx as i32 && mpx < gx + w as i32 && mpy > gy + 16 && mpy < gy + h as i32;
        let mouse_within_resize = mpx > gx + w as i32 - 16 && mpx < gx + w as i32 && mpy > gy + h as i32 - 16 && mpy < gy + h as i32;

        self._frame(x, y, w, h, FrameType::Interactable, title);

        if mouse_within_body || mouse_within_bar {
            self.mouse_captured = true;
        }

        if self.mouse_in_frame(mpx, mpy) && input.get_mouse_button_just_pressed(MouseButton::Left) {
            if mouse_within_x {
                return Some(FrameInteraction::Close);
            } else if mouse_within_resize {
                return Some(FrameInteraction::ResizeBegin);
            } else if mouse_within_bar {
                return Some(FrameInteraction::DragBegin);
            } else {
                return Some(FrameInteraction::OtherContentsClicked);
            }
        }

        if self.mouse_in_frame(mpx, mpy) && mouse_within_body {
            if input.scroll != 0.0 {
                return Some(FrameInteraction::Scroll(input.scroll));
            }
        }

        None
    }

    pub fn text(&mut self, x: i32, y: i32, message: &str) {
        self.add_child(UINode {
            children: Vec::new(),
            clip: (0, 0, 100, 100),
            draw: ElementType::TextLabel(TextLabel {
                message: message.to_string(), x: 0, y: 0
            }),
            focus: self.inc_focus, x, y
        });
        self.inc_focus += 1;
    }

    pub fn image(&mut self, x: i32, y: i32, w: u32, h: u32, tx: (u32, u32), tx_size: (u32, u32), texture: &str) {
        self.add_child(UINode {
            children: Vec::new(),
            clip: (0, 0, w, h),
            draw: ElementType::TextureLabel(TextureLabel {
                x: 0, y: 0, w, h, tx: tx.0, ty: tx.1, th: tx_size.1, tw: tx_size.0, texture: texture.to_string()
            }),
            focus: self.inc_focus, x, y
        });
        self.inc_focus += 1;
    }

    pub fn image_button(&mut self, input: &Input, x: i32, y: i32, w: u32, h: u32, tx: (u32, u32), tx_size: (u32, u32), texture: &str) -> bool {
        self.image(x, y, w, h, tx, tx_size, texture);
        let mpx = input.mouse_pos.0 as i32;
        let mpy = input.mouse_pos.1 as i32;
        let gx = x + self.current_global_origin.0;
        let gy = y + self.current_global_origin.1;
        if self.mouse_in_clip_rect(mpx, mpy) && mpx > gx && mpx < gx + w as i32 && mpy > gy && mpy < gy + h as i32 {
            // println!("in");
            self.mouse_captured = true;
            if input.get_mouse_button_just_pressed(MouseButton::Left) {
                return true;
            }
        }

        false
    }

    fn _slider(&mut self, input: &Input, x: i32, y: i32, size: u32, progress: u32, vertical: bool) -> SliderInteraction {
        self.add_child(UINode {
            children: Vec::new(),
            clip: (0, 0, if vertical { 32 } else { size }, if vertical { size } else { 32 }),
            draw: ElementType::Slider(Slider {
                x: 0, y: 0, size, slider_pos: progress, vertical
            }),
            focus: self.inc_focus, x, y
        });
        self.inc_focus += 1;

        let mpx = input.mouse_pos.0 as i32;
        let mpy = input.mouse_pos.1 as i32;
        let mut gx = x + self.current_global_origin.0;
        let mut gy = y + self.current_global_origin.1;

        let progress = if vertical { (mpy - gy).max(0).min(size as i32) as u32 } else { (mpx - gx).max(0).min(size as i32) as u32 };

        let w = if vertical { 40 } else { size };
        let h = if vertical { size } else { 40 };

        if vertical {
            gx -= 20;
        } else {
            gy -= 20;
        }

        if self.mouse_in_clip_rect(mpx, mpy) && mpx > gx && mpx < gx + w as i32 && mpy > gy && mpy < gy + h as i32 {
            if input.get_mouse_button_just_pressed(MouseButton::Left) {
                return SliderInteraction {
                    clicked: true,
                    progress
                };
            }
        }

        SliderInteraction {
            clicked: false, 
            progress
        }
    }

    pub fn slider(&mut self, input: &Input, x: i32, y: i32, size: u32, progress: u32) -> SliderInteraction {
        self._slider(input, x, y, size, progress, false)
    }

    pub fn vertical_slider(&mut self, input: &Input, x: i32, y: i32, size: u32, progress: u32) -> SliderInteraction {
        self._slider(input, x, y, size, progress, true)
    }

    pub fn pop(&mut self) {
        assert!(!self.parent_nodes.is_empty(), "pop() was called on the root node");
        self.current_global_origin.0 -= self.current_node.borrow().x;
        self.current_global_origin.1 -= self.current_node.borrow().y;
        self.current_node = self.parent_nodes.pop().unwrap();
    }

    unsafe fn render_texture_label(label: &TextureLabel, local_offset: (i32, i32), textures: &TextureBank, ui_program: &mut Program, gl: &glow::Context) {
        let texture = textures.get(&label.texture).unwrap();
        gl.bind_texture(glow::TEXTURE_2D, Some(texture.inner));
        ui_program.uniform_2f32("texSize", vec2(texture.width as f32, texture.height as f32), gl);

        ui_program.uniform_2f32("pos", vec2((label.x + local_offset.0) as f32, (label.y + local_offset.1) as f32), gl);
        ui_program.uniform_2f32("scale", vec2(label.w as f32, label.h as f32), gl);
        ui_program.uniform_2f32("texturePos", vec2(label.tx as f32, label.ty as f32), gl);
        ui_program.uniform_2f32("textureScale", vec2(label.tw as f32, label.th as f32), gl);
        gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
    }

    unsafe fn render_nine_cell(nine_cell: &NineCell, local_offset: (i32, i32), textures: &TextureBank, ui_program: &mut Program, gl: &glow::Context) {
        let frame_texture = textures.get("ui_frame").unwrap();
        gl.bind_texture(glow::TEXTURE_2D, Some(frame_texture.inner));
        ui_program.uniform_2f32("texSize", vec2(frame_texture.width as f32, frame_texture.height as f32), gl);
        ui_program.uniform_2f32("textureScale", vec2(16.0, 16.0), gl);
        
        let x = (nine_cell.x + local_offset.0) as f32;
        let y = (nine_cell.y + local_offset.1) as f32;
        let width = nine_cell.w.max(33) as f32 - 32.0;
        let height = nine_cell.h.max(33) as f32 - 32.0;

        // Hire me
        let tx_origin = nine_cell.frame_type.get_texture_origin();
        let mut ty = tx_origin.1 as f32;
        for (y, h) in [(y, 16.0), (y + 16.0, height), (y + 16.0 + height, 16.0)] {
            let mut tx = tx_origin.0 as f32;
            for (x, w) in [(x, 16.0), (x + 16.0, width), (x + 16.0 + width, 16.0)] {
                ui_program.uniform_2f32("pos", vec2(x, y), gl);
                ui_program.uniform_2f32("scale", vec2(w, h), gl);
                ui_program.uniform_2f32("texturePos", vec2(tx, ty), gl);

                gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);

                tx += 16.0;
            }
            ty += 16.0;
        }
    }

    unsafe fn render_text_label(text: &TextLabel, local_offset: (i32, i32), textures: &TextureBank, ui_program: &mut Program, gl: &glow::Context) {
        let font_texture = textures.get("font").unwrap();
        gl.bind_texture(glow::TEXTURE_2D, Some(font_texture.inner));
        ui_program.uniform_2f32("texSize", vec2(font_texture.width as f32, font_texture.height as f32), gl);
        ui_program.uniform_2f32("scale", vec2(6.0, 9.5), gl);
        ui_program.uniform_2f32("textureScale", vec2(6.0, 9.5), gl);

        let mut x = text.x + local_offset.0;
        let mut y = text.y + local_offset.1;

        for char in text.message.chars() {
            if char == '\n' {
                x = text.x + local_offset.0;
                y += 10;
                continue;
            } else if char == ' ' {
                x += 6;
                continue;
            }

            let char_pos = if let Some(index) = FONT_CHARS.find(char) {
                (index % FONT_WIDTH, index / FONT_WIDTH)
            } else {
                (7, 6)
            };

            ui_program.uniform_2f32("pos", vec2(x as f32, y as f32), gl);
            ui_program.uniform_2f32("texturePos", vec2(char_pos.0 as f32 * 6.0, char_pos.1 as f32 * 10.0), gl);
            gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);

            x += 6;
        }   
    }

    unsafe fn render_slider(slider: &Slider, local_offset: (i32, i32), textures: &TextureBank, ui_program: &mut Program, gl: &glow::Context) {
        let x = slider.x + local_offset.0;
        let y = slider.y + local_offset.1;

        let slider_texture = textures.get("slider").unwrap();
        gl.bind_texture(glow::TEXTURE_2D, Some(slider_texture.inner));
        ui_program.uniform_2f32("texSize", vec2(slider_texture.width as f32, slider_texture.height as f32), gl);

        ui_program.uniform_2f32("pos", vec2(x as f32, y as f32), gl);
        ui_program.uniform_2f32("textureScale", vec2(16.0, 16.0), gl);
        ui_program.uniform_2f32("texturePos", vec2(0.0, 0.0), gl);
        if slider.vertical {
            ui_program.uniform_2f32("scale", vec2(8.0, slider.size as f32), gl);
        } else {
            ui_program.uniform_2f32("scale", vec2(slider.size as f32, 8.0), gl);
        }
        gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);

        if slider.vertical {
            ui_program.uniform_2f32("pos", vec2(x as f32 - 16.0, y as f32 + slider.slider_pos as f32 - 6.0), gl);
            ui_program.uniform_2f32("textureScale", vec2(48.0, 15.5), gl);
            ui_program.uniform_2f32("texturePos", vec2(16.0, 0.0), gl);
            ui_program.uniform_2f32("scale", vec2(48.0, 15.5), gl);
            gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
        } else {
            ui_program.uniform_2f32("pos", vec2(x as f32 + slider.slider_pos as f32 - 6.0, (y - 16) as f32), gl);
            ui_program.uniform_2f32("textureScale", vec2(16.0, 47.5), gl);
            ui_program.uniform_2f32("texturePos", vec2(0.0, 16.0), gl);
            ui_program.uniform_2f32("scale", vec2(16.0, 47.5), gl);
            gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
        } 
    }

    /// Returns `None` if the rect would be empty
    fn intersect(a: (i32, i32, u32, u32), b: (i32, i32, u32, u32)) -> Option<(i32, i32, u32, u32)> {
        let ax2 = a.0 + a.2 as i32;
        let ay2 = a.1 + a.3 as i32;
        let bx2 = b.0 + b.2 as i32;
        let by2 = b.1 + b.3 as i32;

        let x1 = a.0.max(b.0);
        let y1 = a.1.max(b.1);
        let x2 = ax2.min(bx2);
        let y2 = ay2.min(by2);

        if x2 > x1 && y2 > y1 {
            Some((
                x1, y1,
                (x2 - x1) as u32,
                (y2 - y1) as u32
            ))
        } else {
            None
        }
    }

    unsafe fn render_node(&self, node: &NodePtr, local_offset: (i32, i32), clip: (i32, i32, u32, u32), textures: &TextureBank, ui_program: &mut Program, gl: &glow::Context) {
        let element = node.borrow();
        gl.scissor(clip.0, self.screen_size.1 as i32 - clip.1 - clip.3 as i32, clip.2 as i32, clip.3 as i32);
        match &element.draw {
            ElementType::TextureLabel(label) => {
                Self::render_texture_label(label, local_offset, textures, ui_program, gl);
            },
            ElementType::NineCell(nine_cell) => {
                Self::render_nine_cell(nine_cell, local_offset, textures, ui_program, gl);
            },
            ElementType::TextLabel(text) => {
                Self::render_text_label(text, local_offset, textures, ui_program, gl);
            },
            ElementType::TitledNineCell(nine_cell, title) => {
                Self::render_nine_cell(nine_cell, local_offset, textures, ui_program, gl);
                Self::render_text_label(&TextLabel {
                    x: nine_cell.x + 4, y: nine_cell.y + 2,
                    message: title.to_owned()
                }, local_offset, textures, ui_program, gl);
            },
            ElementType::Slider(slider) => {
                Self::render_slider(slider, local_offset, textures, ui_program, gl);
            }
            ElementType::None => ()
        }
    }

    unsafe fn traverse_render(&self, node: &NodePtr, clip: (i32, i32, u32, u32), mut local_offset: (i32, i32), textures: &TextureBank, ui_program: &mut Program, gl: &glow::Context) {
        local_offset.0 += node.borrow().x;
        local_offset.1 += node.borrow().y;
        self.render_node(&node, local_offset, clip, textures, ui_program, gl);
        for child_node in node.borrow().children.iter() {
            let mut new_clip = node.borrow().clip.clone();
            new_clip.0 += local_offset.0;
            new_clip.1 += local_offset.1;
            let intersect = Self::intersect(clip, new_clip);
            if let Some(intersect) = intersect {
                self.traverse_render(child_node, intersect, local_offset, textures, ui_program, gl);
            }
        }
    }

    pub unsafe fn render(&self, textures: &TextureBank, programs: &mut ProgramBank, gl: &glow::Context) {
        gl.disable(glow::CULL_FACE);
        gl.disable(glow::DEPTH_TEST);
        gl.enable(glow::SCISSOR_TEST);

        let ui_program = programs.get_mut("ui").unwrap();

        gl.use_program(Some(ui_program.inner));
        ui_program.uniform_2f32("screenSize", vec2(self.screen_size.0 as f32, self.screen_size.1 as f32), gl);
        ui_program.uniform_1i32("tex", 0, gl);
        // core profile requires a vao bound when drawing arrays even though ui shader is attributeless
        gl.bind_vertex_array(Some(self.vao));
        gl.active_texture(glow::TEXTURE0);

        self.tree.borrow_mut().sort_all();

        self.traverse_render(&self.tree, (0, 0, self.screen_size.0, self.screen_size.1), (0, 0), textures, ui_program, gl);

        gl.enable(glow::CULL_FACE);
        gl.enable(glow::DEPTH_TEST);
        gl.disable(glow::SCISSOR_TEST);
    }
}

pub mod implement {
    use core::f32;

    use cgmath::vec3;
    use winit::event::MouseButton;

    use crate::{common::round_to, input::Input, mesh::flags, shader::ProgramBank, texture::TextureBank, ui::{FrameInteraction, SliderInteraction, FONT_CHARS, UI}, world::{Renderable, World, APPLICABLE_MATERIALS}};

    const MATERIAL_FRAME_SIZE: u32 = 100;

    pub struct VicepticaUI {
        pub inner: UI,
        editor: EditorModeUI,
        play: PlayModeUI,
        /// true - play mode, false - editor
        pub play_mode: bool
    }

    #[derive(PartialEq)]
    enum EditorWindowType {
        Test,
        MaterialPicker,
        LightEditor
    }

    impl EditorWindowType {
        fn title(&self) -> &str {
            match self {
                Self::Test => "Test",
                Self::MaterialPicker => "Materials",
                Self::LightEditor => "Light Properties"
            }
        }
    }

    struct EditorWindow {
        window_type: EditorWindowType,
        position: (i32, i32),
        scale: (u32, u32),
        dragging: bool,
        scaling: bool,
        drag_origin: (i32, i32),
        scale_origin: (u32, u32),
        offset: (f32, f32),
        scroll_max: f32,
        focus: u32,
        sliders: SliderManager
    }

    impl EditorWindow {
        pub fn new(window_type: EditorWindowType, pos: (i32, i32), scale: (u32, u32)) -> Self {
            Self {
                dragging: false,
                scaling: false,
                position: pos,
                scale,
                window_type,
                drag_origin: (0, 0),
                scale_origin: (0, 0),
                offset: (0.0, 0.0),
                scroll_max: 10000.0,
                focus: 0,
                sliders: SliderManager::new()
            }
        }

        fn slider(&mut self, input: &Input, x: i32, y: i32, size: u32, ui: &mut UI) -> u32 {
            let progress = *self.sliders.slider_levels.get(self.sliders.current_slider).unwrap_or(&0);
            self.sliders.add_slider(ui.slider(input, x, y, size, progress));
            progress
        }

        fn vertical_slider(&mut self, input: &Input, x: i32, y: i32, size: u32, ui: &mut UI) -> u32 {
            let progress = *self.sliders.slider_levels.get(self.sliders.current_slider).unwrap_or(&0);
            self.sliders.add_slider(ui.vertical_slider(input, x, y, size, progress));
            size - progress
        }
    }

    impl VicepticaUI {
        pub fn new(gl: &glow::Context) -> Self {
            Self {
                inner: unsafe { UI::new(gl) },
                editor: EditorModeUI::new(),
                play: PlayModeUI::new(),
                play_mode: true
            }
        }

        pub unsafe fn init(&mut self, textures: &mut TextureBank, programs: &mut ProgramBank, gl: &glow::Context) {
            programs.load_by_name_vf("ui", gl).unwrap();
            textures.load_by_name("ui_buttons", gl).unwrap();
            textures.load_by_name("ui_frame", gl).unwrap();
            textures.load_by_name("font", gl).unwrap();
            textures.load_by_name("slider", gl).unwrap();
        }

        pub unsafe fn render_and_update(&mut self, input: &Input, textures: &mut TextureBank, programs: &mut ProgramBank, gl: &glow::Context, world: &mut World) {
            if self.play_mode {
                // todo
            } else {
                self.editor.render_and_update(input, textures, programs, gl, &mut self.inner, world);
            }
        }
    }

    struct EditorModeUI {
        windows: Vec<EditorWindow>,
        mouse_action_origin: (f64, f64),
        highest_focus: u32
    }

    impl EditorModeUI {
        pub fn new() -> Self {
            Self {
                mouse_action_origin: (0.0, 0.0),
                windows: vec![EditorWindow::new(EditorWindowType::LightEditor, (100, 100), (400, 400))],
                highest_focus: 0
            }
        }

        pub fn add_window(&mut self, mut window: EditorWindow) {
            window.focus = self.highest_focus + 1;
            self.highest_focus += 1;
            self.windows.push(window);
        }

        pub fn focus_window(&mut self, window: usize) {
            self.windows[window].focus = self.highest_focus + 1;
            self.highest_focus += 1;
        }

        fn toggle_window(&mut self, window_type: EditorWindowType) {
            let mut open = None;
            for (i, window) in self.windows.iter().enumerate() {
                if window.window_type == window_type {
                    open = Some(i);
                }
            }

            if let Some(i) = open {
                self.windows.remove(i);
            } else {
                self.add_window(EditorWindow::new(window_type, (100, 100), (400, 400)));
            }
        }

        pub unsafe fn render_and_update(&mut self, input: &Input, textures: &mut TextureBank, programs: &mut ProgramBank, gl: &glow::Context, ui: &mut UI, world: &mut World) {
            ui.begin();

            if ui.image_button(&input, 0, 200, 32, 32, (0, 0), (32, 32), "ui_buttons") {
                world.insert_brush(Renderable::Brush(
                    "concrete".to_string(), 
                    vec3(round_to(world.player.position.x, 0.25), round_to(world.player.position.y, 0.25), round_to(world.player.position.z, 0.25)), 
                    vec3(1.0, 1.0, 1.0), 
                    flags::EXTEND_TEXTURE
                ));
            }

            if ui.image_button(&input, 0, 200 + 32, 32, 32, (32, 0), (32, 32), "ui_buttons") {
                self.toggle_window(EditorWindowType::MaterialPicker);
            }
            if ui.image_button(&input, 0, 200 + 64, 32, 32, (64, 0), (32, 32), "ui_buttons") {
                self.toggle_window(EditorWindowType::Test);
            }

            let mut interaction_highest_focus = 0;
            let mut begin_drag = None;
            let mut begin_resize = None;
            let mut close = None;
            let mut scroll = None;
            let mut contents_clicked = None;

            for (i, window) in self.windows.iter_mut().enumerate() {
                if window.dragging {
                    if input.get_mouse_button_released(MouseButton::Left) {
                        window.dragging = false;
                    } else {
                        let diff = (input.mouse_pos.0 - self.mouse_action_origin.0, input.mouse_pos.1 - self.mouse_action_origin.1);
                        window.position = (window.drag_origin.0 + diff.0 as i32, window.drag_origin.1 + diff.1 as i32);
                    }
                }

                if window.scaling {
                    if input.get_mouse_button_released(MouseButton::Left) {
                        window.scaling = false;
                    } else {
                        let diff = (input.mouse_pos.0 - self.mouse_action_origin.0, input.mouse_pos.1 - self.mouse_action_origin.1);
                        window.scale = (
                            (window.scale_origin.0 as i32 + diff.0 as i32).max(48) as u32,
                            (window.scale_origin.1 as i32 + diff.1 as i32).max(48) as u32
                        );
                    }
                }

                window.sliders.reset();

                if let Some(interaction) = ui.interactable_frame(input, window.window_type.title(), window.position.0, window.position.1, window.scale.0, window.scale.1) {
                    if window.focus >= interaction_highest_focus {
                        close = None;
                        begin_drag = None;
                        begin_resize = None;
                        scroll = None;
                        contents_clicked = None;
                        interaction_highest_focus = window.focus;

                        match interaction {
                            FrameInteraction::Close => {
                                close = Some(i);
                            },
                            FrameInteraction::DragBegin => {
                                begin_drag = Some(i);
                                window.drag_origin = window.position;
                            },
                            FrameInteraction::OtherContentsClicked => {
                                contents_clicked = Some(i);
                            },
                            FrameInteraction::ResizeBegin => {
                                begin_resize = Some(i);
                                window.scale_origin = window.scale;
                            },
                            FrameInteraction::Scroll(offset) => {
                                scroll = Some((i, offset));
                            }
                        }
                    }
                }
                ui.set_focus(window.focus);

                let ox = window.offset.0 as i32;
                let oy = window.offset.1 as i32;
                match window.window_type {
                    EditorWindowType::Test => {
                        ui.text(ox + 10, oy + 20, "Hello Everyone\nI will be talking today\n\"Hahahaha\"\n - Me");
                        ui.text(ox + 10, oy + 80, FONT_CHARS);

                        let mut y = oy + 100;
                        for (name, texture) in textures.textures.iter() {
                            ui.text(ox + 10, y, name);
                            y += 15;
                            ui.image(ox + 10, y, texture.width, texture.height, (0, 0), (texture.width, texture.height), name);
                            y += texture.height as i32 + 5;
                        }
                    },
                    EditorWindowType::MaterialPicker => {
                        let rows = (window.scale.0 / MATERIAL_FRAME_SIZE).max(1);
                        let mut x = 0;
                        let mut y = 16;

                        for (i, material) in APPLICABLE_MATERIALS.iter().enumerate() {
                            let texture = textures.textures.get(*material).unwrap();
                            ui.frame(x, y, MATERIAL_FRAME_SIZE, MATERIAL_FRAME_SIZE);
                            let draw_pos = MATERIAL_FRAME_SIZE / 2 - 32;
                            if ui.image_button(input, draw_pos as i32, draw_pos as i32, 64, 64, (0, 0), (texture.width, texture.height), *material) {
                                world.editor_data.apply_material = Some(material.to_string());
                            }
                            ui.pop();
                            if (i + 1) % rows as usize == 0 {
                                x = 0;
                                y += MATERIAL_FRAME_SIZE as i32;
                            } else {
                                x += MATERIAL_FRAME_SIZE as i32;
                            }
                        }
                    },
                    EditorWindowType::LightEditor => {
                        // let progress = window.slider(input, 100, 100, 100, ui);

                        println!("{}", window.vertical_slider(input, 10, 50, 200, ui));
                        // println!("{}", progress);
                    }
                }
                window.sliders.end_of_loop(input);

                ui.pop();
            }

            if let Some(close) = close {
                self.windows.remove(close);
            }

            if let Some(drag) = begin_drag {
                self.windows[drag].dragging = true;
                self.mouse_action_origin = input.mouse_pos;
                self.focus_window(drag);
            }

            if let Some(resize) = begin_resize {
                self.windows[resize].scaling = true;
                self.mouse_action_origin = input.mouse_pos;
                self.focus_window(resize);
            }

            if let Some((i, offset)) = scroll {
                self.windows[i].offset.1 = (self.windows[i].offset.1 - offset).min(0.0).max(-self.windows[i].scroll_max);
            }

            if let Some(clicked) = contents_clicked {
                self.focus_window(clicked);
            }

            ui.render(textures, programs, &gl);
        }
    }

    struct SliderManager {
        slider_levels: Vec<u32>,
        active_slider: Option<usize>,
        sliders: Vec<SliderInteraction>,
        current_slider: usize
    }

    impl SliderManager {
        fn reset(&mut self) {
            // self.sliders.clear();
            self.current_slider = 0;
        }

        fn add_slider(&mut self, interaction: SliderInteraction) {
            if interaction.clicked {
                self.active_slider = Some(self.current_slider);
            }

            if self.current_slider >= self.sliders.len() {
                self.sliders.push(interaction);
                self.slider_levels.push(0);
            } else {
                self.sliders[self.current_slider] = interaction;
            }
            
            self.current_slider += 1;
        }

        fn end_of_loop(&mut self, input: &Input) {
            if input.get_mouse_button_released(MouseButton::Left) {
                self.active_slider = None;
            }

            if let Some(slider) = self.active_slider {
                self.slider_levels[slider] = self.sliders[slider].progress;
            }
        }

        fn new() -> Self {
            Self {
                active_slider: None,
                current_slider: 0,
                slider_levels: Vec::new(),
                sliders: Vec::new()
            }
        }
    }

    struct PlayModeUI {

    }

    impl PlayModeUI {
        pub fn new() -> Self {
            Self {}
        }
    }
}