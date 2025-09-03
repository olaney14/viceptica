use cgmath::{Matrix4, SquareMatrix, Vector3, Zero};
use serde::{Deserialize, Serialize};

use crate::{collision::{self, DEFAULT_CONTROL, DEFAULT_FRICTION, DEFAULT_JUMP}, component::Component, mesh::{self, MeshBank}, render::{self, DirLight, Environment, Skybox}, shader::ProgramBank, texture::TextureBank, world::{self, Model, World}};

#[derive(Deserialize, Serialize)]
pub struct BrushData {
    material: String,
    origin: [f32; 3],
    extents: [f32; 3],
    flags: u32
}

#[derive(Deserialize, Serialize)]
pub struct DirLightData {
    direction: [f32; 3],
    ambient: [f32; 3],
    diffuse: [f32; 3],
    specular: [f32; 3]
}

#[derive(Deserialize, Serialize)]
pub struct EnvironmentData {
    skybox: render::Skybox,
    dir_light: DirLightData
}

#[derive(Deserialize, Serialize)]
pub struct LevelData {
    models: Vec<ModelData>,
    brushes: Vec<BrushData>,
    gravity: f32,
    air_friction: f32,
    materials: Vec<MaterialData>,
    environment: Option<EnvironmentData>
}

#[derive(Deserialize, Serialize)]
pub struct MaterialData {
    name: String,
    diffuse: String,
    specular: String,
    shininess: f32,
    physical_properties: collision::PhysicalProperties
}

#[derive(Deserialize, Serialize, Debug)]
pub enum ModelRenderableData {
    Mesh(String, [[f32; 4]; 4], u32),
    Brush(String, [f32; 3], [f32; 3], u32),
    Billboard(String, [f32; 3], [f32; 2], u32, bool)
}

impl ModelRenderableData {
    pub fn from_renderable(renderable: &world::Renderable) -> Self {
        match renderable {
            world::Renderable::Mesh(name, transform, flags) => {
                Self::Mesh(name.to_owned(), (*transform).into(), *flags)
            },
            world::Renderable::Brush(material, origin, extents, flags) => {
                Self::Brush(material.to_owned(), (*origin).into(), (*extents).into(), *flags)
            },
            world::Renderable::Billboard(texture, origin, size, flags, follow_vertical) => {
                Self::Billboard(texture.to_owned(), (*origin).into(), [size.0, size.1], *flags, *follow_vertical)
            }
        }
    }

    pub fn as_renderable(&self) -> world::Renderable {
        match self {
            Self::Mesh(name, transform, flags) => {
                world::Renderable::Mesh(name.to_owned(), (*transform).into(), *flags)
            },
            Self::Brush(material, origin, extents, flags) => {
                world::Renderable::Brush(material.to_owned(), (*origin).into(), (*extents).into(), *flags)
            },
            Self::Billboard(texture, origin, size, flags, follow_vertical) => {
                world::Renderable::Billboard(texture.to_owned(), (*origin).into(), (size[0], size[1]), *flags, *follow_vertical)
            }
        }
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub enum ModelColliderData {
    None,
    Singular { collider: ModelColliderDataSingular },
    Multiple { colliders: Vec<ModelColliderDataSingular> }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub enum ModelColliderDataSingular {
    Cuboid { offset: [f32; 3], half_extents: [f32; 3] }
}

impl ModelColliderData {
    pub fn from_model_collider(collider: &world::ModelCollider) -> Self {
        let mut colliders = ModelColliderDataSingular::from_model_collider(collider);
        if colliders.is_empty() {
            Self::None
        } else if colliders.len() == 1 {
            Self::Singular { collider: colliders.pop().unwrap() }
        } else {
            Self::Multiple { colliders }
        }
    }

    pub fn as_model_collider(&self) -> Option<world::ModelCollider> {
        match self {
            Self::None => None,
            Self::Singular { collider } => {
                Some(collider.as_model_collider())
            },
            Self::Multiple { colliders } => {
                Some(world::ModelCollider::Multiple { colliders: colliders.iter().cloned().map(|c| c.as_model_collider()).collect() })
            }
        }
    }
}

impl ModelColliderDataSingular {
    pub fn from_model_collider(collider: &world::ModelCollider) -> Vec<Self> {
        let mut colliders = Vec::new();
        match collider {
            world::ModelCollider::Cuboid { offset, half_extents } => {
                colliders.push(ModelColliderDataSingular::Cuboid { offset: (*offset).into(), half_extents: (*half_extents).into() });
            },
            world::ModelCollider::Multiple { colliders: multiple } => {
                for collider in multiple.iter() {
                    colliders.extend(ModelColliderDataSingular::from_model_collider(collider));
                }
            }
        }
        colliders
    }

    pub fn as_model_collider(&self) -> world::ModelCollider {
        match self {
            Self::Cuboid { offset, half_extents } => {
                world::ModelCollider::Cuboid { offset: (*offset).into(), half_extents: (*half_extents).into() }
            }
        }
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub struct PointLightData {
    attenuation: f32,
    color: [f32; 3]
}

#[derive(Deserialize, Serialize, Debug)]
pub struct ModelData {
    transform: [[f32; 4]; 4],
    mobile: bool,
    foreground: bool,
    solid: bool,
    lights: Vec<([f32; 3], PointLightData)>,
    insert_colliders: ModelColliderData,
    renderables: Vec<ModelRenderableData>,
    #[serde(default="Vec::new")]
    components: Vec<Component>
}

impl ModelData {
    pub fn insert(&self, world: &mut World) {
        let mut render = Vec::new();

        for renderable in self.renderables.iter() {
            render.push(renderable.as_renderable());
        }

        let mut model = world::Model::new(
            self.mobile, self.transform.into(), render
        );

        let model_collider = self.insert_colliders.as_model_collider();
        model.insert_collider = model_collider;

        for light in self.lights.iter() {
            let mut point_light = render::PointLight::default(Vector3::zero());
            point_light.set_attenuation(light.1.attenuation);
            point_light.set_color(light.1.color.into());
            model = model.with_light(world.scene.add_point_light(point_light), light.0.into());
        }

        model.components = self.components.clone();

        world.insert_model(model);
    }
}

impl World {
    pub fn save_data(&self) -> LevelData {
        let mut models = Vec::new();

        for (i, model) in self.models.iter().enumerate() {
            if self.internal.internal_ids.contains(&i) { continue; }
            if let Some(model) = model {
                let transform = model.transform.into();
                let mut lights = Vec::new();

                for light in model.lights.iter() {
                    let light_data = PointLightData {
                        attenuation: self.scene.point_lights[light.1].user_attenuation_or_default(),
                        color: self.scene.point_lights[light.1].user_color_or_default().into()
                    };
                    lights.push((light.0.into(), light_data));
                }

                let insert_colliders = if let Some(insert) = &model.insert_collider {
                    ModelColliderData::from_model_collider(insert)
                } else {
                    ModelColliderData::None
                };

                let mut renderables = Vec::new();

                for renderable in model.render.iter() {
                    renderables.push(ModelRenderableData::from_renderable(renderable));
                }

                models.push(ModelData {
                    foreground: model.foreground,
                    mobile: model.mobile,
                    solid: model.solid,
                    transform,
                    lights,
                    insert_colliders,
                    renderables,
                    components: model.components.clone()
                });
            }
        }

        let mut brushes = Vec::new();

        for brush in self.models[self.internal.brushes].as_ref().unwrap().render.iter() {
            if let world::Renderable::Brush(material, origin, extents, flags) = brush {
                brushes.push(BrushData {
                    extents: (*extents).into(),
                    flags: *flags,
                    material: material.to_owned(),
                    origin: (*origin).into()
                });
            }
        }

        let mut materials = Vec::new();

        for material in self.scene.materials.iter() {
            materials.push(MaterialData {
                diffuse: material.1.diffuse.to_owned(),
                name: material.0.to_owned(),
                physical_properties: material.1.physical_properties,
                shininess: material.1.shininess,
                specular: material.1.specular.to_owned()
            });
        }

        let environment = EnvironmentData {
            skybox: self.scene.environment.skybox.clone(),
            dir_light: DirLightData {
                ambient: self.scene.environment.dir_light.ambient.into(),
                diffuse: self.scene.environment.dir_light.diffuse.into(),
                direction: self.scene.environment.dir_light.direction.into(),
                specular: self.scene.environment.dir_light.ambient.into()
            }
        };

        LevelData {
            air_friction: self.air_friction,
            gravity: self.gravity,
            brushes,
            models,
            materials,
            environment: Some(environment)
        }
    }

    pub unsafe fn from_save_data(data: LevelData, textures: &mut TextureBank, meshes: &mut MeshBank, programs: &mut ProgramBank, gl: &glow::Context) -> Self {
        let mut world = world::World::new();
        world.init(meshes, gl);
        for material in data.materials.iter() {
            if !world.scene.materials.contains_key(&material.name) {
                world.scene.load_material_diff_spec_phys(
                    &material.name,
                    &material.diffuse,
                    &material.specular,
                    material.physical_properties,
                    textures,
                    gl
                );
            }
        }

        for model in data.models.iter() {
            model.insert(&mut world);
        }

        let mut brushes = Model::new(false, Matrix4::identity(), Vec::new());

        for brush in data.brushes.iter() {
            brushes.render.push(world::Renderable::Brush(brush.material.to_owned(), brush.origin.into(), brush.extents.into(), brush.flags));
        }

        {
            let environment = data.environment.unwrap_or(EnvironmentData {
                dir_light: DirLightData {
                    ambient: [0.3, 0.3, 0.3],
                    diffuse: [0.6, 0.6, 0.6],
                    specular: [0.75, 0.75, 0.75],
                    direction: [-0.2, -1.0, -0.3]
                },
                skybox: render::Skybox::Cubemap("field".to_string())
            });

            if let Skybox::Cubemap(cubemap) = &environment.skybox {
                if !textures.cubemaps.contains_key(cubemap) {
                    textures.load_cubemap_by_name(cubemap, gl).unwrap();
                }
            }
            world.scene.environment = Environment {
                dir_light: DirLight {
                    ambient: environment.dir_light.ambient.into(),
                    diffuse: environment.dir_light.diffuse.into(),
                    direction: environment.dir_light.direction.into(),
                    specular: environment.dir_light.specular.into()
                },
                skybox: environment.skybox.clone()
            };
        }

        world.scene.init(textures, meshes, programs, gl);
        world.editor_data.selection_box_vao = Some(mesh::create_selection_cube(gl));
        world.set_internal_brushes(brushes);
        world.set_arrows_visible(false);
        world.move_boxes_far();
        world.move_arrows_far();
        world.set_boxes_visible(false);
        world.set_model_visible(world.internal.debug_arrow, false);
        world.freeze = 1;

        world
    }
}

#[derive(Deserialize)]
pub struct BrushMaterialsFile {
    pub materials: Vec<BrushMaterialData> 
}

fn default_specular() -> String { String::from("magic_pixel") }
fn default_friction() -> f32 { DEFAULT_FRICTION }
fn default_control() -> f32 { DEFAULT_CONTROL }
fn default_jump() -> f32 { DEFAULT_JUMP }

#[derive(Deserialize, Debug)]
pub struct BrushMaterialData {
    pub name: String,
    pub diffuse: String,
    #[serde(default="default_specular")]
    pub specular: String,
    #[serde(default="default_friction")]
    pub friction: f32,
    #[serde(default="default_control")]
    pub control: f32,
    #[serde(default="default_jump")]
    pub jump: f32
}