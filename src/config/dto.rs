use super::vector::Vector2D;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ArenaDTO {
    pub id: String,
    pub width: f32,
    pub height: f32,
    pub spawn_margin: f32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FighterDTO {
    pub id: String,
    pub max_health: f32,
    pub max_stamina: f32,
    pub move_speed: f32,
    pub weight: f32,
    pub collider_size: Vector2D,
}
