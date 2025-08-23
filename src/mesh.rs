use std::{collections::HashMap, error::Error, path::PathBuf};

use glow::{HasContext, NativeBuffer, NativeVertexArray};
use itertools::izip;

pub struct Mesh {
    pub vao: NativeVertexArray,
    pub vao_instanced: NativeVertexArray,
    vbo: NativeBuffer,
    ebo: NativeBuffer,
    pub indices: usize,
    pub material: String
}

pub type VertexComponent = f32;
pub type IndexComponent = u16;

pub mod flags {
    pub const NONE: u32 =               0b0000;
    pub const EXTEND_TEXTURE: u32 =     0b0001;
    pub const FULLBRIGHT: u32 =         0b0010;
    pub const SKIP: u32 =               0b0100;
    pub const CUTOUT: u32 =             0b1000;
}

const VERTEX_ATTRIBUTES_COUNT: u32 = 4;

impl Mesh {
    pub fn load_from_obj_vcolor(name: &str, r: VertexComponent, g: VertexComponent, b: VertexComponent, gl: &glow::Context) -> Result<Vec<Self>, Box<dyn Error>> {
        let path = PathBuf::from(format!("res/models/{}.obj", name));
        let (models, _) = tobj::load_obj(
            &path,
            &tobj::LoadOptions {
                triangulate: true,
                single_index: true,
                ..Default::default()
            }
        ).expect(&format!("Failed to load obj file {}", name));

        let mut meshes = Vec::new();

        for model in models.iter() {
            let mesh = &model.mesh;

            // x, y, z, r, g, b, tx, ty, nx, ny, nz
            let mut mesh_data = Vec::new();

            assert!(mesh.positions.len() > 0, "Mesh had no vertices");
            assert!(mesh.texcoords.len() > 0, "Mesh had no texcoords");
            assert!(mesh.normals.len() > 0, "Mesh had no normals");

            for (position, texture_coord, normal) in izip!(mesh.positions.chunks(3), mesh.texcoords.chunks(2), mesh.normals.chunks(3)) {
                mesh_data.extend_from_slice(&[
                    position[0], position[1], position[2],
                    r, g, b,
                    texture_coord[0], texture_coord[1],
                    normal[0], normal[1], normal[2]
                ]);
            }

            meshes.push(unsafe { Self::from_data(&mesh_data, &mesh.indices.iter().map(|i| *i as u16).collect::<Vec<IndexComponent>>(), gl) });
        }

        Ok(meshes)
    }

    pub fn load_from_obj(name: &str, gl: &glow::Context) -> Result<Vec<Self>, Box<dyn Error>> {
        Self::load_from_obj_vcolor(name, 1.0, 1.0, 1.0, gl)
    }

    /// Expected layout: x, y, z, r, g, b, tx, ty, nx, ny, nz
    unsafe fn from_data(vertices: &[VertexComponent], indices: &[IndexComponent], gl: &glow::Context) -> Self {
        let vertices_u8: &[u8] = core::slice::from_raw_parts(
            vertices.as_ptr() as *const u8,
            vertices.len() * core::mem::size_of::<VertexComponent>()
        );
        let indices_u8: &[u8] = core::slice::from_raw_parts(
            indices.as_ptr() as *const u8,
            vertices.len() * core::mem::size_of::<IndexComponent>()
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
            indices: indices.len(),
            material: "default".to_string()
        }
    }

    pub unsafe fn create_cube(gl: &glow::Context) -> Self {
        Self::from_data(&CUBE_VERTICES, &CUBE_INDICES, gl)
    }

    pub unsafe fn create_colored_cube(r: VertexComponent, g: VertexComponent, b: VertexComponent, gl: &glow::Context) -> Self {
        let mut verts = CUBE_VERTICES.to_vec();

        for i in 0..24 {
            verts[i * 11 + 3] = r;
            verts[i * 11 + 4] = g;
            verts[i * 11 + 5] = b;
        }

        Self::from_data(&verts, &CUBE_INDICES, gl)
    }

    pub unsafe fn create_material_cube(material: &str, gl: &glow::Context) -> Self {
        let mut cube = Self::create_cube(gl);
        cube.material = material.to_string();
        cube
    }

    pub unsafe fn create_square(r: VertexComponent, g: VertexComponent, b: VertexComponent, gl: &glow::Context) -> Self {
        Self::from_data(&[
             0.5,  0.5, 0.0, r, g, b, 0.99, 0.99, 0.0, 0.0, -1.0,
             0.5, -0.5, 0.0, r, g, b, 0.99, 0.01, 0.0, 0.0, -1.0,
            -0.5, -0.5, 0.0, r, g, b, 0.01, 0.01, 0.0, 0.0, -1.0,
            -0.5,  0.5, 0.0, r, g, b, 0.01, 0.99, 0.0, 0.0, -1.0
        ], &[
            1, 0, 3,
            3, 2, 1
        ], gl)
    }

    pub fn with_material(mut self, material: &str) -> Self {
        self.material = material.to_string();
        self
    }

    pub unsafe fn create_material_square(material: &str, gl: &glow::Context) -> Self {
        let mut square = Self::create_square(1.0, 1.0, 1.0, gl);
        square.material = material.to_string();
        square
    }

    unsafe fn define_vertex_attributes(gl: &glow::Context) {
        let sizeof_float = core::mem::size_of::<f32>() as i32;
        let stride = 11 * sizeof_float;
        // position
        gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, stride, 0);
        gl.enable_vertex_attrib_array(0);
        // vertex color
        gl.vertex_attrib_pointer_f32(1, 3, glow::FLOAT, false, stride, 3 * sizeof_float);
        gl.enable_vertex_attrib_array(1);
        // texture coordinate
        gl.vertex_attrib_pointer_f32(2, 2, glow::FLOAT, false, stride, 6 * sizeof_float);
        gl.enable_vertex_attrib_array(2);
        // normal
        gl.vertex_attrib_pointer_f32(3, 3, glow::FLOAT, false, stride, 8 * sizeof_float);
        gl.enable_vertex_attrib_array(3);
    }

    pub unsafe fn define_instanced_vertex_attributes(gl: &glow::Context) {
        let u32_size = core::mem::size_of::<u32>() as i32;
        let vec4_size = core::mem::size_of::<cgmath::Vector4<f32>>() as i32;
        let vec3_size = core::mem::size_of::<cgmath::Vector3<f32>>() as i32;
        let stride = u32_size + 4 * vec4_size + 3 * vec3_size;
        gl.enable_vertex_attrib_array(VERTEX_ATTRIBUTES_COUNT);
        gl.vertex_attrib_pointer_i32(VERTEX_ATTRIBUTES_COUNT, 1, glow::UNSIGNED_INT, stride, 0);

        // instance model mat4
        gl.enable_vertex_attrib_array(VERTEX_ATTRIBUTES_COUNT + 1);
        gl.vertex_attrib_pointer_f32(VERTEX_ATTRIBUTES_COUNT + 1, 4, glow::FLOAT, false, stride, u32_size);
        gl.enable_vertex_attrib_array(VERTEX_ATTRIBUTES_COUNT + 2);
        gl.vertex_attrib_pointer_f32(VERTEX_ATTRIBUTES_COUNT + 2, 4, glow::FLOAT, false, stride, 1 * vec4_size + u32_size);
        gl.enable_vertex_attrib_array(VERTEX_ATTRIBUTES_COUNT + 3);
        gl.vertex_attrib_pointer_f32(VERTEX_ATTRIBUTES_COUNT + 3, 4, glow::FLOAT, false, stride, 2 * vec4_size + u32_size);
        gl.enable_vertex_attrib_array(VERTEX_ATTRIBUTES_COUNT + 4);
        gl.vertex_attrib_pointer_f32(VERTEX_ATTRIBUTES_COUNT + 4, 4, glow::FLOAT, false, stride, 3 * vec4_size + u32_size);

        let offset = 4 * vec4_size + u32_size;
        // instance normal matrix mat3
        gl.enable_vertex_attrib_array(VERTEX_ATTRIBUTES_COUNT + 5);
        gl.vertex_attrib_pointer_f32(VERTEX_ATTRIBUTES_COUNT + 5, 3, glow::FLOAT, false, stride, offset);
        gl.enable_vertex_attrib_array(VERTEX_ATTRIBUTES_COUNT + 6);
        gl.vertex_attrib_pointer_f32(VERTEX_ATTRIBUTES_COUNT + 6, 3, glow::FLOAT, false, stride, 1 * vec3_size + offset);
        gl.enable_vertex_attrib_array(VERTEX_ATTRIBUTES_COUNT + 7);
        gl.vertex_attrib_pointer_f32(VERTEX_ATTRIBUTES_COUNT + 7, 3, glow::FLOAT, false, stride, 2 * vec3_size + offset);
    
        // make these properties update per index instead of per vertex
        for i in 0..8 {
            gl.vertex_attrib_divisor(VERTEX_ATTRIBUTES_COUNT + i, 1);
        }
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

    pub fn log_loaded_models(&self) -> String {
        let mut log = String::from("==== MESH BANK ====");
        for (name, mesh) in self.meshes.iter() {
            log.push_str(&format!("\n{}: {} tris, material: {}", name, mesh.indices / 3, mesh.material));
        }
        log.push_str("\n===================");

        log
    }

    pub fn get(&self, name: &str) -> Option<&Mesh> {
        self.meshes.get(name)
    }

    pub fn load_from_obj(&mut self, name: &str, gl: &glow::Context) {
        let meshes = Mesh::load_from_obj(name, gl).expect("Failed to load .obj file");

        for (i, mesh) in meshes.into_iter().enumerate() {
            self.add(mesh, &format!("File_{}{}", name, i));
        }
    }

    pub fn load_from_obj_vcolor(&mut self, file: &str, name: &str, r: VertexComponent, g: VertexComponent, b: VertexComponent, gl: &glow::Context) {
        let meshes = Mesh::load_from_obj_vcolor(file, r, g, b, gl).expect("Failed to load .obj file");

        for (i, mesh) in meshes.into_iter().enumerate() {
            self.add(mesh, &format!("File_{}{}", name, i));
        }
    }
}

// https://pastebin.com/XiCprv6S
const CUBE_VERTICES: [VertexComponent; 264] = [
    // -Z
    -0.5, -0.5, -0.5, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, -1.0,  // A 0
    0.5, -0.5, -0.5, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, -1.0, // B 1
    0.5,  0.5, -0.5, 1.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, -1.0, // C 2
    -0.5,  0.5, -0.5, 1.0, 1.0, 1.0, 0.0, 1.0, 0.0, 0.0, -1.0, // D 3
    // +Z
    -0.5, -0.5,  0.5, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, // E 4
    0.5, -0.5,  0.5, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 1.0, // F 5
    0.5,  0.5,  0.5, 1.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 1.0,  // G 6
    -0.5,  0.5,  0.5, 1.0, 1.0, 1.0, 0.0, 1.0, 0.0, 0.0, 1.0,  // H 7
    // -X
    -0.5,  0.5, -0.5, 1.0, 1.0, 1.0, 0.0, 0.0, -1.0, 0.0, 0.0,  // D 8
    -0.5, -0.5, -0.5, 1.0, 1.0, 1.0, 1.0, 0.0, -1.0, 0.0, 0.0, // A 9
    -0.5, -0.5,  0.5, 1.0, 1.0, 1.0, 1.0, 1.0, -1.0, 0.0, 0.0, // E 10
    -0.5,  0.5,  0.5, 1.0, 1.0, 1.0, 0.0, 1.0, -1.0, 0.0, 0.0, // H 11
    // +X
    0.5, -0.5, -0.5, 1.0, 1.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0,  // B 12
    0.5,  0.5, -0.5, 1.0, 1.0, 1.0, 1.0, 0.0, 1.0, 0.0, 0.0,  // C 13
    0.5,  0.5,  0.5, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0,  // G 14
    0.5, -0.5,  0.5, 1.0, 1.0, 1.0, 0.0, 1.0, 1.0, 0.0, 0.0,  // F 15
    // -Y
    -0.5, -0.5, -0.5, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, -1.0, 0.0, // A 16
    0.5, -0.5, -0.5, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, -1.0, 0.0,  // B 17
    0.5, -0.5,  0.5, 1.0, 1.0, 1.0, 1.0, 1.0, 0.0, -1.0, 0.0,  // F 18
    -0.5, -0.5,  0.5, 1.0, 1.0, 1.0, 0.0, 1.0, 0.0, -1.0, 0.0, // E 19
    // +Y
    0.5,  0.5, -0.5, 1.0, 1.0, 1.0,  0.0, 0.0, 0.0, 1.0, 0.0, // C 20
    -0.5,  0.5, -0.5, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 1.0, 0.0, // D 21
    -0.5,  0.5,  0.5, 1.0, 1.0, 1.0, 1.0, 1.0, 0.0, 1.0, 0.0, // H 22
    0.5,  0.5,  0.5, 1.0, 1.0, 1.0,  0.0, 1.0, 0.0, 1.0, 0.0, // G 23
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

pub unsafe fn create_selection_cube(gl: &glow::Context) -> NativeVertexArray {
    let vertices_u8: &[u8] = core::slice::from_raw_parts(
        SELECTION_CUBE_VERTICES.as_ptr() as *const u8,
        SELECTION_CUBE_VERTICES.len() * core::mem::size_of::<VertexComponent>()
    );
    let indices_u8: &[u8] = core::slice::from_raw_parts(
        SELECTION_CUBE_INDICES.as_ptr() as *const u8,
        SELECTION_CUBE_INDICES.len() * core::mem::size_of::<IndexComponent>()
    );

    let vao = gl.create_vertex_array().unwrap();
    let vbo = gl.create_buffer().unwrap();
    let ebo = gl.create_buffer().unwrap();

    gl.bind_vertex_array(Some(vao));

    gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
    gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, vertices_u8, glow::STATIC_DRAW);
    gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(ebo));
    gl.buffer_data_u8_slice(glow::ELEMENT_ARRAY_BUFFER, indices_u8, glow::STATIC_DRAW);

    gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, 3 * core::mem::size_of::<f32>() as i32, 0);
    gl.enable_vertex_attrib_array(0);

    gl.bind_vertex_array(None);

    vao
}

const SELECTION_CUBE_VERTICES: [VertexComponent; 24] = [
    -0.5,  0.5,  0.5,       // 0
     0.5,  0.5,  0.5,       // 1
    -0.5, -0.5,  0.5,       // 2
     0.5, -0.5,  0.5,       // 3
    -0.5,  0.5,  -0.5,      // 4
     0.5,  0.5,  -0.5,      // 5
    -0.5, -0.5,  -0.5,      // 6
     0.5, -0.5,  -0.5,      // 7
];

const SELECTION_CUBE_INDICES: [IndexComponent; 24] = [
    0, 1,
    1, 3,
    3, 2,
    2, 0,

    0, 4,
    1, 5,
    2, 6,
    3, 7,

    0+4, 1+4,
    1+4, 3+4,
    3+4, 2+4,
    2+4, 0+4,
];

pub unsafe fn create_skybox(gl: &glow::Context) -> NativeVertexArray {
    let vertices_u8: &[u8] = core::slice::from_raw_parts(
        SKYBOX_VERTICES.as_ptr() as *const u8,
        SKYBOX_VERTICES.len() * core::mem::size_of::<VertexComponent>()
    );

    let vao = gl.create_vertex_array().unwrap();
    let vbo = gl.create_buffer().unwrap();

    gl.bind_vertex_array(Some(vao));

    gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
    gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, vertices_u8, glow::STATIC_DRAW);

    gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, 3 * core::mem::size_of::<f32>() as i32, 0);
    gl.enable_vertex_attrib_array(0);

    gl.bind_vertex_array(None);

    vao
}

const SKYBOX_VERTICES: [VertexComponent; 108] = [
    // positions          
    -1.0,  1.0, -1.0,
    -1.0, -1.0, -1.0,
     1.0, -1.0, -1.0,
     1.0, -1.0, -1.0,
     1.0,  1.0, -1.0,
    -1.0,  1.0, -1.0,

    -1.0, -1.0,  1.0,
    -1.0, -1.0, -1.0,
    -1.0,  1.0, -1.0,
    -1.0,  1.0, -1.0,
    -1.0,  1.0,  1.0,
    -1.0, -1.0,  1.0,

     1.0, -1.0, -1.0,
     1.0, -1.0,  1.0,
     1.0,  1.0,  1.0,
     1.0,  1.0,  1.0,
     1.0,  1.0, -1.0,
     1.0, -1.0, -1.0,

    -1.0, -1.0,  1.0,
    -1.0,  1.0,  1.0,
     1.0,  1.0,  1.0,
     1.0,  1.0,  1.0,
     1.0, -1.0,  1.0,
    -1.0, -1.0,  1.0,

    -1.0,  1.0, -1.0,
     1.0,  1.0, -1.0,
     1.0,  1.0,  1.0,
     1.0,  1.0,  1.0,
    -1.0,  1.0,  1.0,
    -1.0,  1.0, -1.0,

    -1.0, -1.0, -1.0,
    -1.0, -1.0,  1.0,
     1.0, -1.0, -1.0,
     1.0, -1.0, -1.0,
    -1.0, -1.0,  1.0,
     1.0, -1.0,  1.0
];