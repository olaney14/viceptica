use cgmath::vec2;
use glow::{HasContext, NativeBuffer, NativeVertexArray};
use winit::event::MouseButton;

use crate::{input::Input, shader::ProgramBank, texture::TextureBank};

const NINECELL_BLOCK: (u32, u32) = (0, 48);
const FONT_BLOCK: (u32, u32) = (0, 128);
pub const NEW_BRUSH: (u32, u32) = (48, 32);
const FONT_CHARS: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789 .!,-�?§µ";
const FONT_WIDTH: usize = 10;
const FONT_HEIGHT: usize = 8;

struct NineCell {
    x: i32,
    y: i32,
    w: u32,
    h: u32
}

struct TextLabel {
    x: i32,
    y: i32,
    message: String
}

struct TextureLabel {
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    tx: u32, ty: u32, tw: u32, th: u32
}

enum UIElement {
    NineCell(NineCell),
    TextLabel(TextLabel),
    TextureLabel(TextureLabel)
}

impl UIElement {
    pub fn bake(&self, start_index: u16, atlas_h: u32, screen_h: u32, vertices: &mut Vec<f32>, indices: &mut Vec<u16>) -> u16 {
        match self {
            UIElement::NineCell(nine_cell) => {
                let width = nine_cell.w.max(33) as f32 - 32.0;
                let height = nine_cell.h.max(33) as f32 - 32.0;

                // 0 1   2 3
                // 4 5   6 7
                //
                // 8 9   0 1
                // 2 3   4 5

                let x = nine_cell.x as f32;
                let x = [x, x + 16.0, x + 16.0 + width, x + 16.0 + width + 16.0];
                let y = (screen_h as i32 - nine_cell.y) as f32;
                let y = [y, y - 16.0, y - 16.0 - height, y - 16.0 - height - 16.0];

                let tx = NINECELL_BLOCK.0 as f32;
                let tx = [tx, tx + 16.0, tx + 32.0, tx + 48.0];
                let ty = (atlas_h - NINECELL_BLOCK.1) as f32;
                let ty = [ty, ty + 16.0, ty + 32.0, ty + 48.0];

                vertices.extend_from_slice(&[
                    x[0], y[0], tx[0], ty[0],
                    x[1], y[0], tx[1], ty[0],
                    x[2], y[0], tx[2], ty[0],
                    x[3], y[0], tx[3], ty[0],

                    x[0], y[1], tx[0], ty[1],
                    x[1], y[1], tx[1], ty[1],
                    x[2], y[1], tx[2], ty[1],
                    x[3], y[1], tx[3], ty[1],

                    x[0], y[2], tx[0], ty[2],
                    x[1], y[2], tx[1], ty[2],
                    x[2], y[2], tx[2], ty[2],
                    x[3], y[2], tx[3], ty[2],

                    x[0], y[3], tx[0], ty[3],
                    x[1], y[3], tx[1], ty[3],
                    x[2], y[3], tx[2], ty[3],
                    x[3], y[3], tx[3], ty[3],
                ]);

                indices.extend([
                    0,  1,  4,  1,  5,  4,
                    1,  2,  5,  2,  6,  5,
                    2,  3,  6,  3,  7,  6,
                    4,  5,  8,  5,  9,  8,
                    5,  6,  9,  6,  10, 9,
                    6,  7,  10, 7,  11, 10,
                    8,  9,  12, 9,  13, 12,
                    9,  10, 13, 10, 14, 13,
                    10, 11, 14, 11, 15, 14
                ].iter().map(|i| i + start_index));

                return 16;
            },
            UIElement::TextLabel(label) => {
                let mut text_vertices = Vec::new();
                let mut text_indices = Vec::new();
                let mut index_offset = start_index;

                let mut x = label.x as f32;
                let mut y = (screen_h as i32 - label.y) as f32;

                for char in label.message.chars() {
                    if char == '\n' {
                        x = label.x as f32;
                        y -= 10.0;
                        continue;
                    } else if char == ' ' {
                        x += 6.0;
                        continue;
                    }

                    let mut char_pos = if let Some(index) = FONT_CHARS.find(char) {
                        (index % FONT_WIDTH, index / FONT_WIDTH)
                    } else {
                        (7, 6)
                    };

                    char_pos.1 = FONT_HEIGHT - char_pos.1 - 1;
                    let tx = FONT_BLOCK.0 as f32 + char_pos.0 as f32 * 6.0;
                    let ty = (atlas_h - FONT_BLOCK.1) as f32 + char_pos.1 as f32 * 10.0;

                    // 3 2
                    // 0 1

                    text_vertices.extend_from_slice(&[
                        x, y, tx, ty,
                        x + 6.0, y, tx + 6.0, ty,
                        x + 6.0, y + 10.0, tx + 6.0, ty + 10.0,
                        x, y + 10.0, tx, ty + 10.0
                    ]);

                    text_indices.extend_from_slice(&[
                        1, 0, 3, 3, 2, 1
                    ].map(|i| i + index_offset));
                    index_offset += 4;

                    x += 6.0;
                }

                vertices.extend(text_vertices.into_iter());
                indices.extend(text_indices.into_iter());
                return index_offset as u16;
            }
            UIElement::TextureLabel(label) => {
                let x = label.x as f32;
                let y = screen_h as f32 - label.y as f32;
                let w = label.w as f32;
                let h = label.h as f32;
                let tx = label.tx as f32;
                let ty = atlas_h as f32 - label.ty as f32;
                let tw = label.tw as f32;
                let th = label.th as f32;  

                vertices.extend_from_slice(&[
                    x, y, tx, ty,
                    x + w, y, tx + tw, ty,
                    x + w, y + h, tx + tw, ty + th,
                    x, y + h, tx, ty + th
                ]);

                indices.extend_from_slice(&[
                    1, 0, 3, 3, 2, 1
                ].map(|i| i + start_index));

                return 4;
            }
        }
    }
}

pub struct UI {
    elements: Vec<UIElement>,
    pub vao: NativeVertexArray,
    vbo: NativeBuffer,
    ebo: NativeBuffer,
    container_stack: Vec<usize>,
    origin: (i32, i32),
    pub screen_size: (u32, u32),
    pub indices: i32,
    pub atlas_height: u32,
}

impl UI {
    pub unsafe fn new(textures: &mut TextureBank, gl: &glow::Context) -> Self {
        let vbo = gl.create_buffer().unwrap();
        let ebo = gl.create_buffer().unwrap();
        let vao = gl.create_vertex_array().unwrap();
        textures.load_by_name("ui_atlas", gl).expect("Failed to load ui atlas");
        Self {
            vao, vbo, ebo,
            elements: Vec::new(),
            container_stack: Vec::new(),
            origin: (0, 0),
            screen_size: (640 * 2, 480 * 2),
            indices: 0,
            atlas_height: textures.get("ui_atlas").unwrap().height
        }
    }

    pub fn begin(&mut self) {
        self.elements.clear();
        self.origin = (0, 0);
        self.container_stack.clear();
    }

    pub fn frame(&mut self, x: i32, y: i32, w: u32, h: u32) {
        self.elements.push(UIElement::NineCell(NineCell {
            x: x + self.origin.0, y: y + self.origin.1, w, h
        }));
        self.container_stack.push(self.elements.len() - 1);
        self.origin = (x + self.origin.0, y + self.origin.1);
    }

    pub fn text(&mut self, x: i32, y: i32, message: &str) {
        self.elements.push(UIElement::TextLabel(TextLabel {
            x: x + self.origin.0, y: y + self.origin.1, message: message.to_string()
        }));
    }

    pub fn image(&mut self, x: i32, y: i32, w: u32, h: u32, tx: (u32, u32), tx_size: (u32, u32)) {
        self.elements.push(UIElement::TextureLabel(TextureLabel {
            x, y: y + h as i32, w, h,
            tx: tx.0, ty: tx.1,
            th: tx_size.1, tw: tx_size.0
        }));
    }

    pub fn image_button(&mut self, input: &Input, x: i32, y: i32, w: u32, h: u32, tx: (u32, u32), tx_size: (u32, u32)) -> bool {
        self.image(x, y, w, h, tx, tx_size);
        if input.get_mouse_button_just_pressed(MouseButton::Left) {
            let mpx = input.mouse_pos.0 as i32;
            let mpy = input.mouse_pos.1 as i32;
            return mpx > x && mpx < x + w as i32 && mpy > y && mpy < y + h as i32;
        }

        false
    }

    pub fn pop(&mut self) {
        self.container_stack.pop();
        if self.container_stack.len() == 0 {
            self.origin = (0, 0);
        } else {
            self.origin = match self.elements.get(*self.container_stack.last().unwrap()).unwrap() {
                UIElement::NineCell(nine_cell) => (nine_cell.x, nine_cell.y),
                UIElement::TextLabel(label) => (label.x, label.y),
                UIElement::TextureLabel(label) => (label.x, label.y)
            };
        }
    }

    pub unsafe fn bake(&mut self, gl: &glow::Context) {
        let mut index = 0;
        let mut indices = Vec::new();
        let mut vertices = Vec::new();

        for elem in self.elements.iter() {
            index += elem.bake(index, self.atlas_height, self.screen_size.1, &mut vertices, &mut indices);
        }
        self.indices = indices.len() as i32;

        let vertices_u8: &[u8] = core::slice::from_raw_parts(
            vertices.as_ptr() as *const u8,
            vertices.len() * core::mem::size_of::<f32>()
        );

        let indices_u8: &[u8] = core::slice::from_raw_parts(
            indices.as_ptr() as *const u8,
            indices.len() * core::mem::size_of::<u16>()
        );

        gl.bind_vertex_array(Some(self.vao));

        gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));
        gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, vertices_u8, glow::STREAM_DRAW);

        gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(self.ebo));
        gl.buffer_data_u8_slice(glow::ELEMENT_ARRAY_BUFFER, indices_u8, glow::STREAM_DRAW);

        let sizeof_float = core::mem::size_of::<f32>() as i32;
        let stride = 4 * sizeof_float;
        gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, stride, 0);
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, stride, 2 * sizeof_float);
        gl.enable_vertex_attrib_array(1);

        gl.bind_vertex_array(None);
    }

    pub unsafe fn render(&self, textures: &TextureBank, programs: &mut ProgramBank, gl: &glow::Context) {
        let ui_program = programs.get_mut("ui").unwrap();
        let atlas_texture = textures.get("ui_atlas").expect("UI atlas was not loaded");
        gl.use_program(Some(ui_program.inner));
        ui_program.uniform_2f32("screenSize", vec2(self.screen_size.0 as f32, self.screen_size.1 as f32), gl);
        ui_program.uniform_2f32("atlasSize", vec2(atlas_texture.width as f32, atlas_texture.height as f32), gl);
        ui_program.uniform_1i32("atlas", 0, gl);
        gl.active_texture(glow::TEXTURE0);
        gl.bind_texture(glow::TEXTURE_2D, Some(atlas_texture.inner));

        gl.disable(glow::DEPTH_TEST);

        gl.bind_vertex_array(Some(self.vao));

        gl.draw_elements(
            glow::TRIANGLES,
            self.indices,
            glow::UNSIGNED_SHORT,
            0
        );

        gl.enable(glow::DEPTH_TEST);
    }
}