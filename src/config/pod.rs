use super::vector::Vector2D;
use static_assertions::{assert_eq_align, assert_eq_size};

#[repr(C, align(64))]
#[derive(Copy, Clone, Debug)]
pub struct ArenaConfigPOD {
    pub id: [u8; 32],      // Offset 0..32
    pub width: f32,        // Offset 32..36
    pub height: f32,       // Offset 36..40
    pub spawn_margin: f32, // Offset 40..44
    pub _pad: [u8; 20],    // Offset 44..64
}

impl Default for ArenaConfigPOD {
    fn default() -> Self {
        Self {
            id: [0; 32],
            width: 0.0,
            height: 0.0,
            spawn_margin: 0.0,
            _pad: [0; 20],
        }
    }
}

#[repr(C, align(64))]
#[derive(Copy, Clone, Debug)]
pub struct FighterAttributesPOD {
    pub id: [u8; 32],            // Offset 0..32
    pub max_health: f32,         // Offset 32..36
    pub max_stamina: f32,        // Offset 36..40
    pub move_speed: f32,         // Offset 40..44
    pub weight: f32,             // Offset 44..48
    pub collider_size: Vector2D, // Offset 48..56 (8 bytes, align 8)
    pub _pad: [u8; 8],           // Offset 56..64
}

impl Default for FighterAttributesPOD {
    fn default() -> Self {
        Self {
            id: [0; 32],
            max_health: 0.0,
            max_stamina: 0.0,
            move_speed: 0.0,
            weight: 0.0,
            collider_size: Vector2D::new(0.0, 0.0),
            _pad: [0; 8],
        }
    }
}

#[repr(C, align(64))]
struct Align64Marker;

assert_eq_size!(ArenaConfigPOD, [u8; 64]);
assert_eq_align!(ArenaConfigPOD, Align64Marker);

assert_eq_size!(FighterAttributesPOD, [u8; 64]);
assert_eq_align!(FighterAttributesPOD, Align64Marker);
