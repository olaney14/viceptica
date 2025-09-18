use std::{fs::File, io::Read, path::Path};

use cgmath::{vec3, Matrix4, Rad};
use itertools::Itertools;
use serde_json as json;

use crate::{component::{self, Component, Trigger, TriggerType}, mesh::{flags, MeshBank}, texture::TextureBank, world::{self, Renderable, World}};

pub const HIDDEN_DEFAULT: bool = false;
pub const SOLID_DEFAULT: bool = false;
pub const FOREGROUND_DEFAULT: bool = false;
pub const MOBILE_DEFAULT: bool = true;
pub const POSITION_DEFAULT: [f32; 3] = [0.0; 3];
pub const ROTATION_DEFAULT: [f32; 3] = [0.0; 3];
pub const SCALE_DEFAULT: [f32; 3] = [1.0; 3];
pub const BRUSH_SCALE_DEFAULT: [f32; 3] = [1.0; 3];
pub const BRUSH_MATERIAL_DEFAULT: &'static str = "rust";
pub const COMMON_SHININESS_DEFAULT: f32 = 1.0;

const ROOT_RECOGNIZED_KEYWORDS: [&'static str; 10] = [
    "hidden", "solid", "foreground", "mobile", "position",
    "scale", "rotation", "render", "component", "__COMMENT__"
];

#[derive(Debug)]
enum PrefabTransform {
    Matrix([[f32; 4]; 4]),
    Component {
        position: [f32; 3],
        scale: [f32; 3],
        rotation: [f32; 3]
    }
}

fn parse_kernel(json: &json::Value) -> Result<[[f32; 3]; 3], String> {
    if !json.is_array() { return Err(String::from("Error at prefab kernel: expected a kernel [[f32; 3]; 3]")); }
    if json.as_array().as_ref().unwrap().len() != 3 { return Err(String::from("Error at prefab kernel: kernel did not have 3 rows")); }

    let mut kernel_vec = Vec::new();
    for row in json.as_array().as_ref().unwrap().iter() {
        if !row.is_array() { return Err(String::from("Error at prefab kernel: a kernel row was not an array")); }
        if row.as_array().as_ref().unwrap().len() != 3 { return Err(String::from("Error at prefab kernel: a kernel row was not 3 elements long")); }

        let mut numbers = row
            .as_array().as_ref().unwrap()
            .iter().map(|e| e.as_f64().unwrap_or_else(|| { 
                eprintln!("Warning at prefab transform: matrix element was not a number"); 0.0 
            })).map(|f| f as f32);

        kernel_vec.push(
            numbers.next_array().unwrap()
        );
    }

    Ok(kernel_vec.into_iter().next_array().unwrap())
}

impl PrefabTransform {
    pub fn parse_matrix(matrix: &json::Value) -> Result<Self, String> {
        if !matrix.is_array() { return Err(String::from("Error at prefab transform: expected a matrix [[f32; 4]; 4]")); }
        if matrix.as_array().as_ref().unwrap().len() != 4 { return Err(String::from("Error at prefab transform: matrix did not have 4 rows")); }

        let mut matrix_vec = Vec::new();
        for row in matrix.as_array().as_ref().unwrap().iter() {
            if !row.is_array() { return Err(String::from("Error at prefab transform: a matrix row was not an array")); }
            if row.as_array().as_ref().unwrap().len() != 4 { return Err(String::from("Error at prefab transform: a matrix row was not 4 elements long")); }

            let mut numbers = row
                .as_array().as_ref().unwrap()
                .iter().map(|e| e.as_f64().unwrap_or_else(|| { 
                    eprintln!("Warning at prefab transform: matrix element was not a number"); 0.0 
                })).map(|f| f as f32);

            matrix_vec.push(
                numbers.next_array().unwrap()
            );
        }

        Ok(Self::Matrix(matrix_vec.into_iter().next_array().unwrap()))
    }

    pub fn parse_components(within: &json::Value) -> Self {
        let position = get_f32_array_or_default(within, "position", POSITION_DEFAULT);
        let rotation = get_f32_array_or_default(within, "rotation", ROTATION_DEFAULT);
        let scale = get_f32_array_or_default(within, "scale", SCALE_DEFAULT);

        Self::Component { position, scale, rotation }
    }

    pub fn parse_within(within: &json::Value) -> Result<Self, String> {
        if let Some(matrix) = within.get("transform") {
            Self::parse_matrix(matrix)
        } else {
            Ok(Self::parse_components(within))
        }
    }

    pub fn as_matrix(&self) -> Matrix4<f32> {
        match self {
            Self::Matrix(matrix) => (*matrix).into(),
            Self::Component { position, scale, rotation } => {
                Matrix4::from_translation(vec3(position[0], position[1], position[2])) *
                Matrix4::from_angle_x(Rad(rotation[0])) * 
                Matrix4::from_angle_y(Rad(rotation[1])) * 
                Matrix4::from_angle_z(Rad(rotation[2])) *
                Matrix4::from_nonuniform_scale(scale[0], scale[1], scale[2])
            }
        }
    }
}

fn parse_render_flags(flags: &json::Value) -> u32 {
    if !flags.is_array() { return 0; }

    let mut flag_aggregate = 0u32;
    for flag in flags.as_array().unwrap() {
        if let json::Value::String(s) = flag {
            match s.as_str() {
                "extend_texture" => flag_aggregate |= flags::EXTEND_TEXTURE,
                "cutout" => flag_aggregate |= flags::CUTOUT,
                "fullbright" => flag_aggregate |= flags::FULLBRIGHT,
                "skip" => flag_aggregate |= flags::SKIP,
                _ => ()
            }
        }
    }

    flag_aggregate
}

#[derive(Debug)]
enum PrefabRenderable {
    Raw(Renderable),
    InsertObj(String, Matrix4<f32>, u32)
}

impl PrefabRenderable {
    pub fn parse(json: &json::Value) -> Result<Self, String> {
        let kind = json
            .get("type").map_or(Err(String::from("Error at prefab render: no type found")), |e| Ok(e))?
            .as_str().map_or(Err(String::from("Error in prefab render: type was not a string")), |e| Ok(e))?;

        match kind {
            "brush" => {
                let origin = get_f32_array_or_default(json, "origin", POSITION_DEFAULT);
                let scale = get_f32_array_or_default(json, "scale", BRUSH_SCALE_DEFAULT);
                // let shininess = get_f32_or_default(json, "shininess", COMMON_SHININESS_DEFAULT);
                let flags = json.get("flags").map(|f| parse_render_flags(f)).unwrap_or(0);
                let material = get_string_or_default(json, "material", BRUSH_MATERIAL_DEFAULT);

                return Ok(PrefabRenderable::Raw(Renderable::Brush(
                    material, origin.into(), scale.into(), flags
                )));
            },
            "mesh" => {
                let transform = PrefabTransform::parse_within(json)?;
                let model = get_string_or_default(json, "mesh", "error"); // TODO
                let flags = json.get("flags").map(|f| parse_render_flags(f)).unwrap_or(0);

                return Ok(PrefabRenderable::Raw(Renderable::Mesh(
                    model, transform.as_matrix(),
                    flags
                )));
            },
            "billboard" => {
                let position = get_f32_array_or_default(json, "position", POSITION_DEFAULT);
                let image = get_string_or_default(json, "image", "error");
                let size = get_f32_array_or_default(json, "size", [1.0; 2]);
                let flags = json.get("flags").map(|f| parse_render_flags(f)).unwrap_or(0);
                let follow_vertical = get_bool_or_default(json, "follow_vertical", false);

                return Ok(PrefabRenderable::Raw(Renderable::Billboard(
                    image, vec3(position[0], position[1], position[2]), (size[0], size[1]),
                    flags, follow_vertical
                )));
            },
            "obj" => {
                let transform = PrefabTransform::parse_within(json)?;
                let obj = get_string_or_default(json, "obj", "error"); // TODO
                let flags = json.get("flags").map(|f| parse_render_flags(f)).unwrap_or(0);

                return Ok(PrefabRenderable::InsertObj(
                    obj, transform.as_matrix(), flags
                ));
            }
            _ => Err(String::from("Error in prefab render: invalid renderable type"))
        }
    }

    pub fn as_renderables(&self, meshes: &MeshBank) -> Vec<world::Renderable> {
        match self {
            Self::Raw(r) => vec![r.clone()],
            Self::InsertObj(object, ..) => {
                let model = world::Model::from_loaded_file(object, meshes).expect("Failed to make model from obj");
                model.render
            }
        }
    }
}

impl Component {
    pub fn parse_from_prefab(json: &json::Value) -> Result<Self, String> {
        let kind = json
            .get("type").map_or(Err(String::from("Error at prefab component: no type found")), |e| Ok(e))?
            .as_str().map_or(Err(String::from("Error in prefab component: type was not a string")), |e| Ok(e))?;

        match kind {
            "spawnpoint" => {
                return Ok(Self::Spawnpoint)
            },
            "door" => {
                let radius = get_f32_or_default(json, "radius", 8.0);
                let height = get_f32_or_default(json, "height", 1.0);
                let open_time = get_i32_or_default(json, "name", 60).abs() as u32;

                return Ok(Self::Door(
                    component::Door::new(radius, height, open_time)
                ))
            },
            "trigger" => {
                let trigger_type = get_string_or_default(json, "trigger", "error");

                let trigger = match trigger_type.as_str() {
                    "fog" => {
                        let color = get_f32_array_or_default(json, "color", [0.5, 0.5, 0.5]);
                        let strength = get_f32_or_default(json, "strength", 64.0);
                        let max = get_f32_or_default(json, "max", 0.75);

                        TriggerType::SetFogEffect { 
                            enabled: true,
                            color, strength, max,
                            max_tween: max
                        }
                    },
                    "kernel" => {
                        if json.get("kernel").is_none() { return Err(String::from("Error in prefab trigger: no kernel specified")); }
                        let kernel = parse_kernel(&json["kernel"])?;
                        let offset = get_f32_or_default(json, "offset", 1.0 / 1300.0);

                        TriggerType::SetKernelEffect { 
                            enabled: true,
                            kernel: [
                                kernel[0][0], kernel[0][1], kernel[0][2],
                                kernel[1][0], kernel[1][1], kernel[1][2],
                                kernel[2][0], kernel[2][1], kernel[2][2]
                            ],
                            offset
                        }
                    },
                    "test" => {
                        let enter = get_string_or_default(json, "enter", "enter");
                        let update = get_string_or_default(json, "update", "update");
                        let exit = get_string_or_default(json, "exit", "exit");

                        TriggerType::Test { enter, update, exit }
                    },
                    _ => return Err(String::from("Error in prefab trigger: invalid trigger type"))
                };

                return Ok(Self::Trigger(Trigger::new(trigger)));
            },
            _ => return Err(String::from("Error in prefab component: invalid component type"))
        }
    }
}

#[derive(Debug)]
pub struct UserPrefab {
    pub hidden: bool,
    pub solid: bool,
    pub foreground: bool,
    pub mobile: bool,
    transform: PrefabTransform,
    pub render: Vec<PrefabRenderable>,
    pub components: Vec<Component>
}

fn get_bool_or_default(json: &json::Value, name: &str, default: bool) -> bool {
    if let Some(value) = json.get(name) {
        value.as_bool().unwrap_or(default)
    } else {
        default
    }
}

fn get_f32_or_default(json: &json::Value, name: &str, default: f32) -> f32 {
    if let Some(value) = json.get(name) {
        value.as_f64().unwrap_or(default as f64) as f32
    } else {
        default
    }
}

fn get_i32_or_default(json: &json::Value, name: &str, default: i32) -> i32 {
    if let Some(value) = json.get(name) {
        value.as_i64().unwrap_or(default as i64) as i32
    } else {
        default
    }
}

fn get_string_or_default<S: AsRef<str>>(json: &json::Value, name: &str, default: S) -> String {
    if let Some(value) = json.get(name) {
        value.as_str().unwrap_or(default.as_ref()).to_owned()
    } else {
        default.as_ref().to_owned()
    }
}

fn get_f32_array_or_default<const N: usize>(json: &json::Value, name: &str, default: [f32; N]) -> [f32; N] {
    if let Some(value) = json.get(name) {
        if let json::Value::Array(values) = value {
            if values.len() != N { return default; }
            let mut slice_values = values.iter().map(|e| e.as_f64().unwrap_or_else(|| { 
                eprintln!("Warning parsing prefab: f32 triple contained non-number"); 0.0 
            })).map(|f| f as f32);
            slice_values.next_array().unwrap()
        } else {
            default
        }
    } else {
        default
    }
}

impl UserPrefab {
    pub fn parse(json: &json::Value) -> Result<Self, String> {
        let hidden = get_bool_or_default(json, "hidden", HIDDEN_DEFAULT);
        let solid = get_bool_or_default(json, "solid", SOLID_DEFAULT);
        let foreground = get_bool_or_default(json, "foreground", FOREGROUND_DEFAULT);
        let mobile = get_bool_or_default(json, "mobile", MOBILE_DEFAULT);
        let transform = PrefabTransform::parse_within(json)?;
        let mut renderables = Vec::new();
        if let Some(json::Value::Array(array)) = json.get("render") {
            for item in array {
                renderables.push(PrefabRenderable::parse(item)?);
            }
        }

        let mut components = Vec::new();
        if let Some(json::Value::Array(array)) = json.get("components") {
            for item in array {
                components.push(Component::parse_from_prefab(item)?);
            }
        }

        Ok(Self {
            hidden, solid, foreground, mobile, transform, render: renderables,
            components
        })
    }

    pub unsafe fn load_resources(&self, world: &mut World, textures: &mut TextureBank, meshes: &mut MeshBank, gl: &glow::Context) {
        let mut requested_textures = Vec::new();
        let mut requested_meshes = Vec::new();

        for renderable in self.render.iter() {
            match renderable {
                PrefabRenderable::Raw(Renderable::Billboard(texture, ..)) => requested_textures.push(texture.to_owned()),
                PrefabRenderable::Raw(Renderable::Mesh(mesh, ..)) => requested_meshes.push(mesh.to_owned()),
                PrefabRenderable::InsertObj(obj, ..) => {
                    meshes.load_from_obj(obj, gl);
                    world.loaded_models.push(obj.to_owned());
                }
                _ => ()
            }
        }

        for mesh in requested_meshes.iter() {
            meshes.load_from_obj(mesh, gl);
            world.loaded_models.push(mesh.to_owned());
        }

        for texture in requested_textures.iter() {
            println!("{}", texture);
            textures.load_by_name(&texture, gl).expect("Could not find texture requested by prefab");
        }
    }

    pub fn as_model(&self, meshes: &MeshBank) -> world::Model {
        let renderables = self.render.iter().map(|r| r.as_renderables(meshes)).flatten();
        let mut model = world::Model::new(self.mobile, self.transform.as_matrix(), renderables.collect());
        model.foreground = self.foreground;
        model.hidden = self.hidden;
        model.solid = self.solid;
        model.components = self.components.clone();
        model
    }
}

impl World {
    pub fn insert_prefab_from_file<P: AsRef<Path>>(&mut self, textures: &mut TextureBank, meshes: &mut MeshBank, gl: &glow::Context, path: P) -> Result<usize, String> {
        let mut file = File::open(path).map_err(|e| e.to_string())?;
        let mut data = String::new();
        file.read_to_string(&mut data).map_err(|e| e.to_string())?;
        let prefab_source = serde_json::from_str(data.as_str()).map_err(|e| e.to_string())?;
        let prefab = UserPrefab::parse(&prefab_source)?;

        unsafe { prefab.load_resources(self, textures, meshes, gl); }
        Ok(self.insert_model(prefab.as_model(meshes)))
    }
}