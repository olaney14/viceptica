pub fn round_to(num: f32, to: f32) -> f32 {
    (num / to).round() * to
}

pub fn floor_to(num: f32, to: f32) -> f32 {
    (num / to).floor() * to
}