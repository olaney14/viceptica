use std::{collections::HashMap, error::Error, path::PathBuf};

use glow::{HasContext, NativeBuffer, NativeVertexArray};

pub struct Mesh {
    pub vao: NativeVertexArray,
    pub vao_instanced: NativeVertexArray,
    vbo: NativeBuffer,
    ebo: NativeBuffer,
    pub indices: usize,
    pub texture: String
}

pub type VertexComponent = f32;
pub type IndexComponent = u16;

impl Mesh {
    // pub fn load_from_file_obj(name: &str, gl: glow::Context) -> Result<Self, Box<dyn Error>> {
    //     let path = PathBuf::from(format!("res/models/{}.obj", name));
    //     let (models, materials) = tobj::load_obj(
    //         &path,
    //         &tobj::LoadOptions::default()
    //     ).expect(&format!("Failed to load obj file {}", name));

    //     let materials = materials.expect("Failed to load MTL file");

    //     for (i, model) in models.iter().enumerate() {
    //         let mesh = &model.mesh;

    //         mesh.
    //     }
    // }
    pub unsafe fn create_cube(gl: &glow::Context) -> Self {
        let vertices_u8: &[u8] = core::slice::from_raw_parts(
            CUBE_VERTICES.as_ptr() as *const u8,
            CUBE_VERTICES.len() * core::mem::size_of::<VertexComponent>()
        );
        let indices_u8: &[u8] = core::slice::from_raw_parts(
            CUBE_INDICES.as_ptr() as *const u8,
            CUBE_INDICES.len() * core::mem::size_of::<IndexComponent>()
        );

        let vao = gl.create_vertex_array().unwrap();
        let vao_instanced = gl.create_vertex_array().unwrap();
        let vbo = gl.create_buffer().unwrap();
        let ebo = gl.create_buffer().unwrap();

        gl.bind_vertex_array(Some(vao));

        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
        gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, vertices_u8, glow::STATIC_DRAW);

        gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(ebo));
        gl.buffer_data_u8_slice(glow::ELEMENT_ARRAY_BUFFER, indices_u8, glow::STATIC_DRAW);

        Self::define_vertex_attributes(gl);
        
        gl.bind_vertex_array(None);

        gl.bind_vertex_array(Some(vao_instanced));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
        gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(ebo));
        Self::define_vertex_attributes(gl);
        gl.bind_vertex_array(None);
        // this vao is left unfinished until static mesh data is ready

        Self {
            vao, vbo, ebo,
            vao_instanced,
            indices: CUBE_INDICES.len(),
            texture: "magic_pixel".to_string()
        }
    }

    pub unsafe fn create_textured_cube(texture: &str, gl: &glow::Context) -> Self {
        let mut cube = Self::create_cube(gl);
        cube.texture = texture.to_string();
        cube
    }

    pub unsafe fn create_square(r: VertexComponent, g: VertexComponent, b: VertexComponent, gl: &glow::Context) -> Self {
        let vertices: Vec<VertexComponent> = vec![
             0.5,  0.5, 0.0, r, g, b, 1.0, 1.0,
             0.5, -0.5, 0.0, r, g, b, 1.0, 0.0,
            -0.5, -0.5, 0.0, r, g, b, 0.0, 0.0,
            -0.5,  0.5, 0.0, r, g, b, 0.0, 1.0
        ];
        let vertices_u8: &[u8] = core::slice::from_raw_parts(
            vertices.as_ptr() as *const u8,
            vertices.len() * core::mem::size_of::<VertexComponent>()
        );

        let indices: Vec<IndexComponent> = vec![
            0, 1, 3,
            1, 2, 3
        ];
        let indices_u8: &[u8] = core::slice::from_raw_parts(
            indices.as_ptr() as *const u8,
            indices.len() * core::mem::size_of::<IndexComponent>()
        );

        let vao = gl.create_vertex_array().unwrap();
        let vao_instanced = gl.create_vertex_array().unwrap();
        let vbo = gl.create_buffer().unwrap();
        let ebo = gl.create_buffer().unwrap();

        gl.bind_vertex_array(Some(vao));

        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
        gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, vertices_u8, glow::STATIC_DRAW);

        gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(ebo));
        gl.buffer_data_u8_slice(glow::ELEMENT_ARRAY_BUFFER, indices_u8, glow::STATIC_DRAW);

        Self::define_vertex_attributes(gl);
        
        gl.bind_vertex_array(None);

        gl.bind_vertex_array(Some(vao_instanced));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
        gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(ebo));
        Self::define_vertex_attributes(gl);
        gl.bind_vertex_array(None);
        // this vao is left unfinished until static mesh data is ready

        Self {
            vao, vbo, ebo,
            vao_instanced,
            indices: 6,
            texture: "magic_pixel".to_string()
        }
    }

    pub unsafe fn create_textured_square(texture: &str, gl: &glow::Context) -> Self {
        let mut square = Self::create_square(1.0, 1.0, 1.0, gl);
        square.texture = texture.to_string();
        square
    }

    unsafe fn define_vertex_attributes(gl: &glow::Context) {
        let sizeof_float = core::mem::size_of::<f32>() as i32;
        // position
        gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, 8 * sizeof_float, 0);
        gl.enable_vertex_attrib_array(0);
        // vertex color
        gl.vertex_attrib_pointer_f32(1, 3, glow::FLOAT, false, 8 * sizeof_float, 3 * sizeof_float);
        gl.enable_vertex_attrib_array(1);
        // texture coordinate
        gl.vertex_attrib_pointer_f32(2, 2, glow::FLOAT, false, 8 * sizeof_float, 6 * sizeof_float);
        gl.enable_vertex_attrib_array(2);
    }

    pub unsafe fn define_instanced_vertex_attributes(gl: &glow::Context) {
        let vec4_size = core::mem::size_of::<cgmath::Vector4<f32>>() as i32;
        // instance model mat4
        gl.enable_vertex_attrib_array(3);
        gl.vertex_attrib_pointer_f32(3, 4, glow::FLOAT, false, 4 * vec4_size, 0);
        gl.enable_vertex_attrib_array(4);
        gl.vertex_attrib_pointer_f32(4, 4, glow::FLOAT, false, 4 * vec4_size, 1 * vec4_size);
        gl.enable_vertex_attrib_array(5);
        gl.vertex_attrib_pointer_f32(5, 4, glow::FLOAT, false, 4 * vec4_size, 2 * vec4_size);
        gl.enable_vertex_attrib_array(6);
        gl.vertex_attrib_pointer_f32(6, 4, glow::FLOAT, false, 4 * vec4_size, 3 * vec4_size);
    
        // what does this do ????
        gl.vertex_attrib_divisor(3, 1);
        gl.vertex_attrib_divisor(4, 1);
        gl.vertex_attrib_divisor(5, 1);
        gl.vertex_attrib_divisor(6, 1);
    }
}

pub struct MeshBank {
    pub meshes: HashMap<String, Mesh>
}

impl MeshBank {
    pub fn new() -> Self {
        Self {
            meshes: HashMap::new()
        }
    }

    pub fn add(&mut self, mesh: Mesh, name: &str) {
        if !self.meshes.contains_key(name) {
            self.meshes.insert(name.to_string(), mesh);
        }
    }

    pub fn get(&self, name: &str) -> Option<&Mesh> {
        self.meshes.get(name)
    }
}

// https://pastebin.com/XiCprv6S
const CUBE_VERTICES: [VertexComponent; 192] = [
    -0.5, -0.5, -0.5, 1.0, 1.0, 1.0, 0.0, 0.0,  // A 0
    0.5, -0.5, -0.5, 1.0, 1.0, 1.0, 1.0, 0.0,  // B 1
    0.5,  0.5, -0.5, 1.0, 1.0, 1.0, 1.0, 1.0,  // C 2
    -0.5,  0.5, -0.5, 1.0, 1.0, 1.0, 0.0, 1.0,  // D 3
    -0.5, -0.5,  0.5, 1.0, 1.0, 1.0, 0.0, 0.0,  // E 4
    0.5, -0.5,  0.5, 1.0, 1.0, 1.0, 1.0, 0.0,   // F 5
    0.5,  0.5,  0.5, 1.0, 1.0, 1.0, 1.0, 1.0,   // G 6
    -0.5,  0.5,  0.5, 1.0, 1.0, 1.0, 0.0, 1.0,   // H 7

    -0.5,  0.5, -0.5, 1.0, 1.0, 1.0, 0.0, 0.0,  // D 8
    -0.5, -0.5, -0.5, 1.0, 1.0, 1.0, 1.0, 0.0,  // A 9
    -0.5, -0.5,  0.5, 1.0, 1.0, 1.0, 1.0, 1.0,  // E 10
    -0.5,  0.5,  0.5, 1.0, 1.0, 1.0, 0.0, 1.0,  // H 11
    0.5, -0.5, -0.5, 1.0, 1.0, 1.0, 0.0, 0.0,   // B 12
    0.5,  0.5, -0.5, 1.0, 1.0, 1.0, 1.0, 0.0,   // C 13
    0.5,  0.5,  0.5, 1.0, 1.0, 1.0, 1.0, 1.0,   // G 14
    0.5, -0.5,  0.5, 1.0, 1.0, 1.0, 0.0, 1.0,   // F 15

    -0.5, -0.5, -0.5, 1.0, 1.0, 1.0, 0.0, 0.0,  // A 16
    0.5, -0.5, -0.5, 1.0, 1.0, 1.0, 1.0, 0.0,   // B 17
    0.5, -0.5,  0.5, 1.0, 1.0, 1.0, 1.0, 1.0,   // F 18
    -0.5, -0.5,  0.5, 1.0, 1.0, 1.0, 0.0, 1.0,  // E 19
    0.5,  0.5, -0.5, 1.0, 1.0, 1.0,  0.0, 0.0,  // C 20
    -0.5,  0.5, -0.5, 1.0, 1.0, 1.0, 1.0, 0.0,  // D 21
    -0.5,  0.5,  0.5, 1.0, 1.0, 1.0, 1.0, 1.0,  // H 22
    0.5,  0.5,  0.5, 1.0, 1.0, 1.0,  0.0, 1.0,  // G 23
];

const CUBE_INDICES: [IndexComponent; 36] = [
    // front and back
    0, 3, 2,
    2, 1, 0,
    4, 5, 6,
    6, 7 ,4,
    // left and right
    11, 8, 9,
    9, 10, 11,
    12, 13, 14,
    14, 15, 12,
    // bottom and top
    16, 17, 18,
    18, 19, 16,
    20, 21, 22,
    22, 23, 20
];