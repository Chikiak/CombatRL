use serde::{Deserialize, Serialize};
use static_assertions::{assert_eq_align, assert_eq_size};

#[repr(C, align(8))]
#[derive(Copy, Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Vector2D {
    pub x: f32,
    pub y: f32,
}

impl Vector2D {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[repr(C, align(8))]
struct Align8Marker;

assert_eq_size!(Vector2D, [u8; 8]);
assert_eq_align!(Vector2D, Align8Marker);
