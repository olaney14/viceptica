use core::f32;

use cgmath::{vec3, Matrix, Matrix3, Matrix4, SquareMatrix, Vector3};

pub fn round_to(num: f32, to: f32) -> f32 {
    (num / to).round() * to
}

pub fn floor_to(num: f32, to: f32) -> f32 {
    (num / to).floor() * to
}

pub fn mat4_to_mat3(mat: Matrix4<f32>) -> Matrix3<f32> {
    Matrix3::new(
        mat.x.x, mat.x.y, mat.x.z,
        mat.y.x, mat.y.y, mat.y.z,
        mat.z.x, mat.z.y, mat.z.z
    )
}

pub fn mat3_to_mat4(mat: Matrix3<f32>) -> Matrix4<f32> {
    Matrix4::new(
        mat.x.x, mat.x.y, mat.x.z, 0.0,
        mat.y.x, mat.y.y, mat.y.z, 0.0,
        mat.z.x, mat.z.y, mat.z.z, 0.0,
        0.0,   0.0,   0.0,   1.0,
    )   
}

pub fn aabb_to_extents(aabb: &parry3d::bounding_volume::Aabb) -> (Vector3<f32>, Vector3<f32>) {
    let center = aabb.center();
    let half_extents = aabb.half_extents();
    (
        vec3(center.x, center.y, center.z),
        vec3(half_extents.x, half_extents.y, half_extents.z)
    )
}

pub fn mat4_remove_translation(mat: Matrix4<f32>) -> Matrix4<f32> {
    mat3_to_mat4(mat4_to_mat3(mat))
}

pub fn vec3_mix(a: Vector3<f32>, b: Vector3<f32>, t: f32) -> Vector3<f32> {
    a * (1.0 - t) + b * t
}

pub fn translation(mat: Matrix4<f32>) -> Vector3<f32> {
    mat.w.xyz()
}

// https://learnopengl.com/Lighting/Basic-Lighting
pub fn normal_matrix(mat: Matrix4<f32>) -> Matrix3<f32> {
    mat4_to_mat3(mat.invert().unwrap().transpose())
}

pub fn compose_extents<I>(extents: I) -> (Vector3<f32>, Vector3<f32>)
where I: IntoIterator<Item = (Vector3<f32>, Vector3<f32>)> 
{
    let mut min = vec3(f32::MAX, f32::MAX, f32::MAX);
    let mut max = vec3(f32::MIN, f32::MIN, f32::MIN);
    let mut len = 0;

    for (center, size) in extents {
        len += 1;
        let min_corner = center - size;
        let max_corner = center + size;
        min = min.zip(min_corner, |a, b| a.min(b));
        max = max.zip(max_corner, |a, b| a.max(b));
    }

    if len == 0 {
        return (vec3_zero(), vec3_all(0.25))
    }

    (
        (min + max) * 0.5,
        (max - min) * 0.5
    )
}

#[inline]
pub fn vec3_all(of: f32) -> Vector3<f32> {
    vec3(of, of, of)
}

#[inline]
pub fn vec3_zero() -> Vector3<f32> {
    vec3_all(0.0)
}

#[inline]
pub fn vec3_div_compwise(a: Vector3<f32>, b: Vector3<f32>) -> Vector3<f32> {
    vec3(a.x / b.x, a.y / b.y, a.z / b.z)
}

pub fn towards(a: f32, b: f32, by: f32) -> f32 {
    (b - a).signum() * by
}

pub fn fuzzy_eq(a: f32, b: f32, epsilon: f32) -> bool {
    (a - b).abs() < epsilon
}