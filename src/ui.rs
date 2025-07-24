use std::{cell::RefCell, collections::HashMap, rc::Rc};

use cgmath::vec2;
use glow::{HasContext, NativeBuffer, NativeVertexArray};
use winit::event::MouseButton;

use crate::{input::Input, shader::{Program, ProgramBank}, texture::TextureBank};

const FONT_CHARS: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789 .!,- ?  _";
const FONT_WIDTH: usize = 10;
// const FONT_HEIGHT: usize = 8;

// const Z_INCREMENT_MINOR: f32 = 0.0001;
// const Z_INCREMENT_MAJOR: f32 = Z_INCREMENT_MINOR * 100.0;

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
enum ElementType {
    NineCell(NineCell),
    TextLabel(TextLabel),
    TextureLabel(TextureLabel),
    TitledNineCell(NineCell, String),
    None
}

#[derive(Clone, Copy, Debug)]
struct Rect<T: Clone> {
    x: T,
    y: T,
    w: T,
    h: T
}

impl<T: Clone> Rect<T> {
    pub fn new(x: T, y: T, w: T, h: T) -> Self {
        Self { x, y, w, h, }
    }
}

impl Rect<f32> {
    pub fn from_clip_rect(clip: (i32, i32, u32, u32)) -> Self {
        Self::new(clip.0 as f32, clip.1 as f32, clip.2 as f32, clip.3 as f32)
    }
    
    pub fn shifted(mut self, x: f32, y: f32) -> Self {
        self.x += x;
        self.y += y;
        self
    }
}

// struct UIElement {
//     elem: ElementType,
//     z: f32,
//     clip: Rect<f32>
// }

// impl UIElement {
//     pub fn new(elem: ElementType, z: f32) -> Self {
//         Self {
//             elem, z, clip: Rect::new(0.0, 0.0, 1.0, 1.0)
//         }
//     }
// }

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

    // pub fn insert(&mut self, node: UINode) {
    //     self.children.push(Box::new(node));
    // }

    /// Recursively sort all UI nodes
    pub fn sort_all(&mut self) {
        self.children.sort_by(|a, b| a.borrow().focus.cmp(&b.borrow().focus));

        for child in self.children.iter_mut() {
            child.borrow_mut().sort_all();
        }
    }
}

// struct Container {
//     pub index: usize,
//     pub clip_rect: (i32, i32, u32, u32)
// }

pub struct UI {
    // elements: Vec<UIElement>,
    tree: NodePtr,
    current_node: NodePtr,
    parent_nodes: Vec<NodePtr>,
    last_modified: NodePtr,
    pub vao: NativeVertexArray,
    vbo: NativeBuffer,
    ebo: NativeBuffer,
    // container_stack: Vec<Container>,
    // origin: (i32, i32),
    pub screen_size: (u32, u32),
    // pub current_z: f32,
    pub mouse_captured: bool,
    pub inc_focus: u32,
    current_global_origin: (i32, i32)
}

impl UI {
    pub unsafe fn new(textures: &mut TextureBank, gl: &glow::Context) -> Self {
        let vbo = gl.create_buffer().unwrap();
        let ebo = gl.create_buffer().unwrap();
        let vao = gl.create_vertex_array().unwrap();
        // textures.load_by_name("ui_atlas", gl).expect("Failed to load ui atlas");
        let tree = Rc::new(RefCell::new(UINode::root()));
        Self {
            vao, vbo, ebo,
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

    // fn focus(&mut self, focus: u32) {
    //     self.focus = focus;
    // }

    pub fn begin(&mut self) {
        self.tree = Rc::new(RefCell::new(UINode::root()));
        self.current_node = self.tree.clone();
        self.last_modified = self.tree.clone();
        self.parent_nodes.clear();
        self.mouse_captured = false;
        self.current_global_origin = (0, 0);
    }

    // fn set_clip_rect(&mut self) {
    //     self.elements.last_mut().unwrap().clip = if self.container_stack.len() == 0 { 
    //         Rect::new(0.0, 0.0, self.screen_size.0 as f32, self.screen_size.1 as f32) 
    //     } else {
    //         Rect::from_clip_rect(self.container_stack.last().unwrap().clip_rect)
    //     };
    // }

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
                clip: (0, 16, w, h - 16),
                draw: ElementType::NineCell(NineCell {
                    x: 0, y: 0, w, h, frame_type: frame
                }),
                focus: self.inc_focus, x, y
            });
        } else {
            self.add_child_as_current(UINode {
                children: Vec::new(),
                clip: (0, 16, w, h - 16),
                draw: ElementType::TitledNineCell(NineCell {
                    x: 0, y: 0, w, h, frame_type: frame
                }, title.to_string()),
                focus: self.inc_focus, x, y
            });
        }

        // self.last_modified = Some(self.current_node.clone());
        self.inc_focus += 1;
        //*self.current_node.as_ref().unwrap().borrow_mut().children.push(Box::new(x));
        // self.elements.push(UIElement::new(ElementType::NineCell( NineCell {
        //     x: x + self.origin.0, y: y + self.origin.1, w, h, frame_type: frame
        // }), self.current_z));
        // self.set_clip_rect();
        // self.container_stack.push(Container {
        //     index: self.elements.len() - 1,
        //     clip_rect: (x + self.origin.0, y + self.origin.1 + 16, w, h - 16)
        // });
        // self.origin = (x + self.origin.0, y + self.origin.1);
        // self.current_z += Z_INCREMENT_MAJOR;
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
        // if let Some(container) = self.container_stack.last() {
        //     return mpx > container.clip_rect.0 && mpx < container.clip_rect.0 + container.clip_rect.2 as i32
        //         && mpy > container.clip_rect.1 && mpy < container.clip_rect.1 + container.clip_rect.3 as i32
        // }

        // true
    }

    pub fn mouse_in_frame(&self, mpx: i32, mpy: i32) -> bool {
        let clip = self.global_clip_rect();
        mpx > clip.0 && mpx < clip.0 + clip.2 as i32 && mpy > clip.1 - 16 && mpy < clip.1 + clip.3 as i32 + 16
        // if let Some(container) = self.container_stack.last() {
        //     return mpx > container.clip_rect.0 && mpx < container.clip_rect.0 + container.clip_rect.2 as i32
        //         && mpy > container.clip_rect.1 - 16 && mpy < container.clip_rect.1 + container.clip_rect.3 as i32 + 16
        // }

        // false
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

    // pub fn interactable_frame_titled(&mut self, input: &Input, x: i32, y: i32, w: u32, h: u32, title: &str) -> Option<FrameInteraction> {
    //     // self.current_z += Z_INCREMENT_MAJOR;
    //     // self.text(x + 4, y + 2, title);
    //     // self.set_focus(self.inc_focus);
    //     // self.inc_focus -= 1;
    //     // self.current_z -= Z_INCREMENT_MAJOR;
    //     self.interactable_frame(input, x, y, w, h)
    // }

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
        // self.elements.push(UIElement::new(ElementType::TextLabel(TextLabel {
        //     x: x + self.origin.0, y: y + self.origin.1, message: message.to_string()
        // }), self.current_z));
        // self.set_clip_rect();
        // self.current_z += Z_INCREMENT_MINOR;
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
        // self.elements.push(UIElement::new(ElementType::TextureLabel(TextureLabel {
        //     x: x + self.origin.0, y: y + self.origin.1, w, h,
        //     tx: tx.0, ty: tx.1,
        //     th: tx_size.1, tw: tx_size.0, texture: texture.to_string()
        // }), self.current_z));
        // self.set_clip_rect();
        // self.current_z += Z_INCREMENT_MINOR;
    }

    pub fn image_button(&mut self, input: &Input, x: i32, y: i32, w: u32, h: u32, tx: (u32, u32), tx_size: (u32, u32), texture: &str) -> bool {
        self.image(x, y, w, h, tx, tx_size, texture);
        let mpx = input.mouse_pos.0 as i32;
        let mpy = input.mouse_pos.1 as i32;
        if self.mouse_in_clip_rect(mpx, mpy) && mpx > x && mpx < x + w as i32 && mpy > y && mpy < y + h as i32 {
            self.mouse_captured = true;
            if input.get_mouse_button_just_pressed(MouseButton::Left) {
                return true;
            }
        }

        false
    }

    pub fn pop(&mut self) {
        assert!(!self.parent_nodes.is_empty(), "pop() was called on the root node");
        self.current_global_origin.0 -= self.current_node.borrow().x;
        self.current_global_origin.1 -= self.current_node.borrow().y;
        self.current_node = self.parent_nodes.pop().unwrap();
        // self.current_z -= Z_INCREMENT_MAJOR;
        // self.container_stack.pop();
        // if self.container_stack.len() == 0 {
        //     self.origin = (0, 0);
        // } else {
        //     self.origin = match &self.elements.get(self.container_stack.last().unwrap().index).unwrap().elem {
        //         ElementType::NineCell(nine_cell) => (nine_cell.x, nine_cell.y),
        //         ElementType::TextLabel(label) => (label.x, label.y),
        //         ElementType::TextureLabel(label) => (label.x, label.y)
        //     };
        // }
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
            }
            ElementType::None => ()
        }
    }

    unsafe fn traverse_render(&self, node: &NodePtr, clip: (i32, i32, u32, u32), mut local_offset: (i32, i32), textures: &TextureBank, ui_program: &mut Program, gl: &glow::Context) {
        local_offset.0 += node.borrow().x;
        local_offset.1 += node.borrow().y;
        // println!("Rendering: {:?}, clip: {:?}", node.borrow().draw, clip);
        self.render_node(&node, local_offset, clip, textures, ui_program, gl);
        for child_node in node.borrow().children.iter() {
            let mut new_clip = node.borrow().clip.clone();
            new_clip.0 += local_offset.0;
            new_clip.1 += local_offset.1;
            self.traverse_render(child_node, new_clip, local_offset, textures, ui_program, gl);
        }
    }

    pub unsafe fn render(&self, textures: &TextureBank, programs: &mut ProgramBank, gl: &glow::Context) {
        // println!("============ Begin Render =============");
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
        // let frame_texture = textures.get("ui_frame").unwrap();
        // let font_texture = textures.get("font").unwrap();

        self.tree.borrow_mut().sort_all();

        self.traverse_render(&self.tree, (0, 0, self.screen_size.0, self.screen_size.1), (0, 0), textures, ui_program, gl);

        // for element in self.elements.iter() {
        //     ui_program.uniform_1f32("z", element.z, gl);
        //     gl.scissor(element.clip.x as i32, self.screen_size.1 as i32 - element.clip.y as i32 - element.clip.h as i32, element.clip.w as i32, element.clip.h as i32);
        //     match &element.elem {
        //         ElementType::TextureLabel(label) => {
        //             let texture = textures.get(&label.texture).unwrap();
        //             gl.bind_texture(glow::TEXTURE_2D, Some(texture.inner));
        //             ui_program.uniform_2f32("texSize", vec2(texture.width as f32, texture.height as f32), gl);

        //             ui_program.uniform_2f32("pos", vec2(label.x as f32, label.y as f32), gl);
        //             ui_program.uniform_2f32("scale", vec2(label.w as f32, label.h as f32), gl);
        //             ui_program.uniform_2f32("texturePos", vec2(label.tx as f32, label.ty as f32), gl);
        //             ui_program.uniform_2f32("textureScale", vec2(label.tw as f32, label.th as f32), gl);
        //             gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
        //         },
        //         ElementType::NineCell(nine_cell) => {
        //             gl.bind_texture(glow::TEXTURE_2D, Some(frame_texture.inner));
        //             ui_program.uniform_2f32("texSize", vec2(frame_texture.width as f32, frame_texture.height as f32), gl);
        //             ui_program.uniform_2f32("textureScale", vec2(16.0, 16.0), gl);
                    
        //             let width = nine_cell.w.max(33) as f32 - 32.0;
        //             let height = nine_cell.h.max(33) as f32 - 32.0;

        //             // Hire me
        //             let tx_origin = nine_cell.frame_type.get_texture_origin();
        //             let mut ty = tx_origin.1 as f32;
        //             for (y, h) in [(nine_cell.y as f32, 16.0), (nine_cell.y as f32 + 16.0, height), (nine_cell.y as f32 + 16.0 + height, 16.0)] {
        //                 let mut tx = tx_origin.0 as f32;
        //                 for (x, w) in [(nine_cell.x as f32, 16.0), (nine_cell.x as f32 + 16.0, width), (nine_cell.x as f32 + 16.0 + width, 16.0)] {
        //                     ui_program.uniform_2f32("pos", vec2(x, y), gl);
        //                     ui_program.uniform_2f32("scale", vec2(w, h), gl);
        //                     ui_program.uniform_2f32("texturePos", vec2(tx, ty), gl);

        //                     gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);

        //                     tx += 16.0;
        //                 }
        //                 ty += 16.0;
        //             }
        //         },
        //         ElementType::TextLabel(text) => {
        //             gl.bind_texture(glow::TEXTURE_2D, Some(font_texture.inner));
        //             ui_program.uniform_2f32("texSize", vec2(font_texture.width as f32, font_texture.height as f32), gl);
        //             ui_program.uniform_2f32("scale", vec2(6.0, 10.0), gl);
        //             ui_program.uniform_2f32("textureScale", vec2(6.0, 10.0), gl);

        //             let mut x = text.x;
        //             let mut y = text.y;

        //             for char in text.message.chars() {
        //                 if char == '\n' {
        //                     x = text.x;
        //                     y += 10;
        //                     continue;
        //                 } else if char == ' ' {
        //                     x += 6;
        //                     continue;
        //                 }

        //                 let char_pos = if let Some(index) = FONT_CHARS.find(char) {
        //                     (index % FONT_WIDTH, index / FONT_WIDTH)
        //                 } else {
        //                     (7, 6)
        //                 };

        //                 ui_program.uniform_2f32("pos", vec2(x as f32, y as f32), gl);
        //                 ui_program.uniform_2f32("texturePos", vec2(char_pos.0 as f32 * 6.0, char_pos.1 as f32 * 10.0), gl);
        //                 gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);

        //                 x += 6;
        //             }   
        //         }
        //         _ => ()
        //     }
        // }

        gl.enable(glow::CULL_FACE);
        gl.enable(glow::DEPTH_TEST);
        gl.disable(glow::SCISSOR_TEST);

        //println!("================= End Render ================");
        //panic!();
    }
}

pub mod implement {
    use core::f32;

    use cgmath::vec3;
    use winit::event::MouseButton;

    use crate::{common::round_to, input::{self, Input}, mesh::flags, shader::ProgramBank, texture::TextureBank, ui::{FrameInteraction, FONT_CHARS, UI}, world::{Renderable, World}};

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
        MaterialPicker
    }

    impl EditorWindowType {
        fn title(&self) -> &str {
            match self {
                Self::Test => "Test",
                Self::MaterialPicker => "Materials"
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
        focus: u32
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
                focus: 0
            }
        }
    }

    impl VicepticaUI {
        pub fn new(textures: &mut TextureBank, gl: &glow::Context) -> Self {
            Self {
                inner: unsafe { UI::new(textures, gl) },
                editor: EditorModeUI::new(),
                play: PlayModeUI::new(),
                play_mode: true
            }
        }

        pub unsafe fn init(&mut self, textures: &mut TextureBank, programs: &mut ProgramBank, gl: &glow::Context) {
            programs.load_by_name_vf("ui", &gl).unwrap();
            textures.load_by_name("ui_buttons", &gl).unwrap();
            textures.load_by_name("ui_frame", &gl).unwrap();
            textures.load_by_name("font", &gl).unwrap();
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
                windows: Vec::new(),
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
                // let mut materials_open = None;
                // for (i, window) in self.windows.iter().enumerate() {
                //     if matches!(window.window_type, EditorWindowType::MaterialPicker) {
                //         materials_open = Some(i);
                //     }
                // }

                // if let Some(i) = materials_open {
                //     self.windows.remove(i);
                // } else {
                //     self.add_window(EditorWindow::new(EditorWindowType::MaterialPicker, (100, 100), (400, 400)));
                //     // self.windows.push(EditorWindow::new(EditorWindowType::MaterialPicker, (100, 100), (400, 400)));
                // }
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

                    }
                }

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

    struct PlayModeUI {

    }

    impl PlayModeUI {
        pub fn new() -> Self {
            Self {}
        }
    }
}