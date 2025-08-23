use cgmath::{Matrix, Matrix3, Matrix4, SquareMatrix, Vector3};

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

pub fn mat4_remove_translation(mat: Matrix4<f32>) -> Matrix4<f32> {
    mat3_to_mat4(mat4_to_mat3(mat))
}

pub fn vec3_mix(a: Vector3<f32>, b: Vector3<f32>, t: f32) -> Vector3<f32> {
    a * (1.0 - t) + b * t
}

// https://learnopengl.com/Lighting/Basic-Lighting
pub fn normal_matrix(mat: Matrix4<f32>) -> Matrix3<f32> {
    mat4_to_mat3(mat.invert().unwrap().transpose())
}