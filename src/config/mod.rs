pub mod dto;
pub mod error;
pub mod loader;
pub mod pod;
pub mod vector;

pub use dto::{ArenaDTO, FighterDTO};
pub use error::ConfigError;
pub use loader::{
    load_arena, load_default_arena, load_default_fighter, load_fighter, validate_arena_dto,
    validate_fighter_dto,
};
pub use pod::{ArenaConfigPOD, FighterAttributesPOD};
pub use vector::Vector2D;

pub const MAX_SIMULTANEOUS_ENTITIES: usize = 64;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_loading() {
        // Assert exact size and alignment requirements (DoD #2)
        assert_eq!(std::mem::size_of::<ArenaConfigPOD>(), 64);
        assert_eq!(std::mem::align_of::<ArenaConfigPOD>(), 64);
        assert_eq!(std::mem::size_of::<FighterAttributesPOD>(), 64);
        assert_eq!(std::mem::align_of::<FighterAttributesPOD>(), 64);
        assert_eq!(std::mem::size_of::<Vector2D>(), 8);
        assert_eq!(std::mem::align_of::<Vector2D>(), 8);

        let arena = load_default_arena().expect("Failed to load default arena");
        assert_eq!(arena.width, 30.0);
        assert_eq!(arena.height, 18.0);
        assert_eq!(arena.spawn_margin, 2.0);

        let fighter = load_default_fighter().expect("Failed to load default fighter");
        assert_eq!(fighter.max_health, 150.0);
        assert_eq!(fighter.max_stamina, 60.0);
        assert_eq!(fighter.move_speed, 5.0);
        assert_eq!(fighter.weight, 1.0);
        assert_eq!(fighter.collider_size.x, 0.8);
        assert_eq!(fighter.collider_size.y, 1.8);

        // Verify zero-allocation validation & compilation pass (DoD #2)
        // validate_* builds the POD on the stack from a DTO: must be alloc-free.
        let sample_arena_dto = ArenaDTO {
            id: "test_arena".to_string(),
            width: 30.0,
            height: 20.0,
            spawn_margin: 2.0,
        };
        let sample_fighter_dto = FighterDTO {
            id: "test_fighter".to_string(),
            max_health: 100.0,
            max_stamina: 50.0,
            move_speed: 4.0,
            weight: 1.0,
            collider_size: Vector2D::new(0.8, 1.8),
        };
        let validated_arena = alloc_counter::deny_alloc(|| {
            validate_arena_dto(&sample_arena_dto).expect("valid arena DTO")
        });
        assert_eq!(validated_arena.width, 30.0);

        let validated_fighter = alloc_counter::deny_alloc(|| {
            validate_fighter_dto(&sample_fighter_dto).expect("valid fighter DTO")
        });
        assert_eq!(validated_fighter.max_health, 100.0);

        // The loader path performs JSON parsing (heap allocs are expected), but
        // the allocation count must be deterministic across repeated loads.
        let (counts_a, _) = alloc_counter::count_alloc(|| load_default_arena().unwrap());
        let (counts_b, _) = alloc_counter::count_alloc(|| load_default_arena().unwrap());
        assert_eq!(
            counts_a, counts_b,
            "arena load allocation count must be deterministic"
        );

        let (counts_c, _) = alloc_counter::count_alloc(|| load_default_fighter().unwrap());
        let (counts_d, _) = alloc_counter::count_alloc(|| load_default_fighter().unwrap());
        assert_eq!(
            counts_c, counts_d,
            "fighter load allocation count must be deterministic"
        );
    }

    #[test]
    fn test_invalid_bounds_rejection() {
        // Arena invalid width (< 10.0)
        let bad_arena = ArenaDTO {
            id: "small".to_string(),
            width: 8.0,
            height: 20.0,
            spawn_margin: 1.0,
        };
        let res = validate_arena_dto(&bad_arena);
        assert!(matches!(
            res,
            Err(ConfigError::InvalidBounds {
                parameter: "width",
                value: 8.0
            })
        ));

        // Arena invalid height (< 10.0)
        let bad_arena_height = ArenaDTO {
            id: "short".to_string(),
            width: 30.0,
            height: 9.0,
            spawn_margin: 1.0,
        };
        let res = validate_arena_dto(&bad_arena_height);
        assert!(matches!(
            res,
            Err(ConfigError::InvalidBounds {
                parameter: "height",
                value: 9.0
            })
        ));

        // Arena negative spawn_margin (< 0.0)
        let bad_arena_margin = ArenaDTO {
            id: "margin".to_string(),
            width: 30.0,
            height: 20.0,
            spawn_margin: -1.0,
        };
        let res = validate_arena_dto(&bad_arena_margin);
        assert!(matches!(
            res,
            Err(ConfigError::InvalidBounds {
                parameter: "spawn_margin",
                value: -1.0
            })
        ));

        // Arena subnormal float
        let sub_arena = ArenaDTO {
            id: "sub".to_string(),
            width: 1e-40,
            height: 20.0,
            spawn_margin: 1.0,
        };
        let res = validate_arena_dto(&sub_arena);
        assert!(matches!(
            res,
            Err(ConfigError::SubnormalFloatDetected { parameter: "width" })
        ));

        // Arena NaN float
        let nan_arena = ArenaDTO {
            id: "nan".to_string(),
            width: f32::NAN,
            height: 20.0,
            spawn_margin: 1.0,
        };
        let res = validate_arena_dto(&nan_arena);
        assert!(matches!(
            res,
            Err(ConfigError::SubnormalFloatDetected { parameter: "width" })
        ));

        // Arena Infinity float
        let inf_arena = ArenaDTO {
            id: "inf".to_string(),
            width: f32::INFINITY,
            height: 20.0,
            spawn_margin: 1.0,
        };
        let res = validate_arena_dto(&inf_arena);
        assert!(matches!(
            res,
            Err(ConfigError::SubnormalFloatDetected { parameter: "width" })
        ));

        // Fighter invalid health (< 1.0)
        let bad_fighter = FighterDTO {
            id: "weak".to_string(),
            max_health: 0.0,
            max_stamina: 50.0,
            move_speed: 4.0,
            weight: 1.0,
            collider_size: Vector2D::new(1.0, 1.0),
        };
        let res = validate_fighter_dto(&bad_fighter);
        assert!(matches!(
            res,
            Err(ConfigError::InvalidBounds {
                parameter: "max_health",
                value: 0.0
            })
        ));

        // Fighter invalid stamina (< 1.0)
        let bad_stamina = FighterDTO {
            id: "tired".to_string(),
            max_health: 100.0,
            max_stamina: 0.5,
            move_speed: 4.0,
            weight: 1.0,
            collider_size: Vector2D::new(1.0, 1.0),
        };
        let res = validate_fighter_dto(&bad_stamina);
        assert!(matches!(
            res,
            Err(ConfigError::InvalidBounds {
                parameter: "max_stamina",
                value: 0.5
            })
        ));

        // Fighter invalid move_speed (< 0.0)
        let bad_speed = FighterDTO {
            id: "slow".to_string(),
            max_health: 100.0,
            max_stamina: 50.0,
            move_speed: -0.1,
            weight: 1.0,
            collider_size: Vector2D::new(1.0, 1.0),
        };
        let res = validate_fighter_dto(&bad_speed);
        assert!(matches!(
            res,
            Err(ConfigError::InvalidBounds {
                parameter: "move_speed",
                value: -0.1
            })
        ));

        // Fighter invalid weight (< 0.1)
        let bad_weight = FighterDTO {
            id: "feather".to_string(),
            max_health: 100.0,
            max_stamina: 50.0,
            move_speed: 4.0,
            weight: 0.05,
            collider_size: Vector2D::new(1.0, 1.0),
        };
        let res = validate_fighter_dto(&bad_weight);
        assert!(matches!(
            res,
            Err(ConfigError::InvalidBounds {
                parameter: "weight",
                value: 0.05
            })
        ));

        // Fighter invalid collider x (< 0.1)
        let bad_collider = FighterDTO {
            id: "tiny".to_string(),
            max_health: 100.0,
            max_stamina: 50.0,
            move_speed: 4.0,
            weight: 1.0,
            collider_size: Vector2D::new(0.05, 1.0),
        };
        let res = validate_fighter_dto(&bad_collider);
        assert!(matches!(
            res,
            Err(ConfigError::InvalidBounds {
                parameter: "collider_size.x",
                value: 0.05
            })
        ));

        // Fighter invalid collider y (< 0.1)
        let bad_collider_y = FighterDTO {
            id: "tinyy".to_string(),
            max_health: 100.0,
            max_stamina: 50.0,
            move_speed: 4.0,
            weight: 1.0,
            collider_size: Vector2D::new(1.0, 0.04),
        };
        let res = validate_fighter_dto(&bad_collider_y);
        assert!(matches!(
            res,
            Err(ConfigError::InvalidBounds {
                parameter: "collider_size.y",
                value: 0.04
            })
        ));

        // ID too long (> 32 bytes)
        let long_id = "a".repeat(33);
        let long_arena = ArenaDTO {
            id: long_id,
            width: 30.0,
            height: 20.0,
            spawn_margin: 1.0,
        };
        let res = validate_arena_dto(&long_arena);
        assert!(matches!(res, Err(ConfigError::InvalidSchema)));

        // File not found
        let nf = load_arena("nonexistent_arena_file.json");
        assert!(matches!(nf, Err(ConfigError::FileNotFound)));
    }
}
