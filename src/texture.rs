use std::{collections::HashMap, error::Error, path::PathBuf};

use glow::{HasContext, PixelUnpackData};

pub struct Texture {
    pub width: u32,
    pub height: u32,
    pub name: String,
    pub inner: glow::Texture
}

pub struct Cubemap {
    pub name: String,
    pub inner: glow::Texture
}

pub struct TextureBank {
    pub textures: HashMap<String, Texture>,
    pub cubemaps: HashMap<String, Cubemap>
}

impl TextureBank {
    pub unsafe fn load_cubemap_by_name(&mut self, name: &str, gl: &glow::Context) -> Result<(), Box<dyn Error>> {
        let base_path = format!("res/textures/cubemap/{}/", name);

        let id = gl.create_texture()?;
        gl.bind_texture(glow::TEXTURE_CUBE_MAP, Some(id));

        for (i, face) in ["px", "nx", "py", "ny", "pz", "nz"].iter().enumerate() {
            let image_path = PathBuf::from(format!("{}{}.png", base_path, face));
            let image = image::open(image_path)?.to_rgba8();
            let width = image.width();
            let height = image.height();
            let data = image.as_flat_samples();
            let slice = data.as_slice();

            gl.tex_image_2d(
                glow::TEXTURE_CUBE_MAP_POSITIVE_X + i as u32, 
                0, 
                glow::RGBA8 as i32, 
                width as i32, 
                height as i32, 
                0, 
                glow::RGBA, 
                glow::UNSIGNED_BYTE, 
                PixelUnpackData::Slice(Some(slice))
            );
        }

        cubemap_texture_settings(gl);

        self.cubemaps.insert(name.to_string(), Cubemap {
            inner: id,
            name: name.to_string()
        });

        Ok(())
    }

    pub unsafe fn load_by_name(&mut self, name: &str, gl: &glow::Context) -> Result<(), Box<dyn Error>> {
        if self.textures.contains_key(name) {
            return Ok(());
        }
        
        let image_path = PathBuf::from(format!("res/textures/{}.png", name));
        let image = image::open(image_path)?.flipv().to_rgba8();
        let width = image.width();
        let height = image.height();
        let data = image.as_flat_samples();
        let slice = data.as_slice();

        let raw_texture = gl.create_texture()?;
        gl.bind_texture(glow::TEXTURE_2D, Some(raw_texture));

        texture_settings(gl);

        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::RGBA8 as i32,
            width as i32,
            height as i32,
            0,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            PixelUnpackData::Slice(Some(slice))
        );

        gl.generate_mipmap(glow::TEXTURE_2D);
        gl.bind_texture(glow::TEXTURE_2D, None);

        self.textures.insert(name.to_string(), Texture {
            width, height, name: name.to_string(),
            inner: raw_texture
        });

        Ok(())
    }

    pub fn new() -> Self {
        Self {
            textures: HashMap::new(),
            cubemaps: HashMap::new()
        }
    }

    pub fn get(&self, name: &str) -> Option<&Texture> {
        self.textures.get(name)
    }

    pub fn get_cubemap(&self, name: &str) -> Option<&Cubemap> {
        self.cubemaps.get(name)
    }
}

unsafe fn texture_settings(gl: &glow::Context) {
    gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::REPEAT as i32);
    gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::REPEAT as i32);
    gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::NEAREST_MIPMAP_NEAREST as i32);
    gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::NEAREST as i32);
}

unsafe fn cubemap_texture_settings(gl: &glow::Context) {
    gl.tex_parameter_i32(glow::TEXTURE_CUBE_MAP, glow::TEXTURE_MIN_FILTER, glow::LINEAR as i32);
    gl.tex_parameter_i32(glow::TEXTURE_CUBE_MAP, glow::TEXTURE_MAG_FILTER, glow::LINEAR as i32);
    gl.tex_parameter_i32(glow::TEXTURE_CUBE_MAP, glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE as i32);
    gl.tex_parameter_i32(glow::TEXTURE_CUBE_MAP, glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE as i32);
    gl.tex_parameter_i32(glow::TEXTURE_CUBE_MAP, glow::TEXTURE_WRAP_R, glow::CLAMP_TO_EDGE as i32);
}