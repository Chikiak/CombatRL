use super::dto::{ArenaDTO, FighterDTO};
use super::error::ConfigError;
use super::pod::{ArenaConfigPOD, FighterAttributesPOD};
use std::fs;
use std::path::Path;

pub fn encode_id(s: &str) -> Result<[u8; 32], ConfigError> {
    let bytes = s.as_bytes();
    if bytes.len() > 32 {
        return Err(ConfigError::InvalidSchema);
    }
    let mut buf = [0u8; 32];
    buf[..bytes.len()].copy_from_slice(bytes);
    Ok(buf)
}

pub fn validate_f32(v: f32, name: &'static str) -> Result<(), ConfigError> {
    if !v.is_finite() {
        return Err(ConfigError::SubnormalFloatDetected { parameter: name });
    }
    if !v.is_normal() && v != 0.0 {
        return Err(ConfigError::SubnormalFloatDetected { parameter: name });
    }
    Ok(())
}

pub fn validate_arena_dto(dto: &ArenaDTO) -> Result<ArenaConfigPOD, ConfigError> {
    let id_bytes = encode_id(&dto.id)?;

    validate_f32(dto.width, "width")?;
    validate_f32(dto.height, "height")?;
    validate_f32(dto.spawn_margin, "spawn_margin")?;

    if dto.width < 10.0 {
        return Err(ConfigError::InvalidBounds {
            parameter: "width",
            value: dto.width,
        });
    }
    if dto.height < 10.0 {
        return Err(ConfigError::InvalidBounds {
            parameter: "height",
            value: dto.height,
        });
    }
    if dto.spawn_margin < 0.0 {
        return Err(ConfigError::InvalidBounds {
            parameter: "spawn_margin",
            value: dto.spawn_margin,
        });
    }

    Ok(ArenaConfigPOD {
        id: id_bytes,
        width: dto.width,
        height: dto.height,
        spawn_margin: dto.spawn_margin,
        _pad: [0; 20],
    })
}

pub fn validate_fighter_dto(dto: &FighterDTO) -> Result<FighterAttributesPOD, ConfigError> {
    let id_bytes = encode_id(&dto.id)?;

    validate_f32(dto.max_health, "max_health")?;
    validate_f32(dto.max_stamina, "max_stamina")?;
    validate_f32(dto.move_speed, "move_speed")?;
    validate_f32(dto.weight, "weight")?;
    validate_f32(dto.collider_size.x, "collider_size.x")?;
    validate_f32(dto.collider_size.y, "collider_size.y")?;

    if dto.max_health < 1.0 {
        return Err(ConfigError::InvalidBounds {
            parameter: "max_health",
            value: dto.max_health,
        });
    }
    if dto.max_stamina < 1.0 {
        return Err(ConfigError::InvalidBounds {
            parameter: "max_stamina",
            value: dto.max_stamina,
        });
    }
    if dto.move_speed < 0.0 {
        return Err(ConfigError::InvalidBounds {
            parameter: "move_speed",
            value: dto.move_speed,
        });
    }
    if dto.weight < 0.1 {
        return Err(ConfigError::InvalidBounds {
            parameter: "weight",
            value: dto.weight,
        });
    }
    if dto.collider_size.x < 0.1 {
        return Err(ConfigError::InvalidBounds {
            parameter: "collider_size.x",
            value: dto.collider_size.x,
        });
    }
    if dto.collider_size.y < 0.1 {
        return Err(ConfigError::InvalidBounds {
            parameter: "collider_size.y",
            value: dto.collider_size.y,
        });
    }

    Ok(FighterAttributesPOD {
        id: id_bytes,
        max_health: dto.max_health,
        max_stamina: dto.max_stamina,
        move_speed: dto.move_speed,
        weight: dto.weight,
        collider_size: dto.collider_size,
        _pad: [0; 8],
    })
}

pub fn load_arena<P: AsRef<Path>>(path: P) -> Result<ArenaConfigPOD, ConfigError> {
    if !path.as_ref().exists() {
        return Err(ConfigError::FileNotFound);
    }
    let content = fs::read_to_string(path)?;
    let dto: ArenaDTO = serde_json::from_str(&content)?;
    validate_arena_dto(&dto)
}

pub fn load_fighter<P: AsRef<Path>>(path: P) -> Result<FighterAttributesPOD, ConfigError> {
    if !path.as_ref().exists() {
        return Err(ConfigError::FileNotFound);
    }
    let content = fs::read_to_string(path)?;
    let dto: FighterDTO = serde_json::from_str(&content)?;
    validate_fighter_dto(&dto)
}

pub fn load_default_arena() -> Result<ArenaConfigPOD, ConfigError> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let path = Path::new(manifest_dir).join("assets/configs/arenas/default_arena.json");
    load_arena(path)
}

pub fn load_default_fighter() -> Result<FighterAttributesPOD, ConfigError> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let path = Path::new(manifest_dir).join("assets/configs/fighters/default_fighter.json");
    load_fighter(path)
}
