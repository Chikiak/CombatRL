use crate::config::{Vector2D, MAX_SIMULTANEOUS_ENTITIES};
use crate::prng::PrngStatePOD;
use static_assertions::{assert_eq_align, assert_eq_size};
use thiserror::Error;
use xxhash_rust::xxh3::xxh3_64;

#[derive(Debug, PartialEq, Error)]
pub enum StateError {
    #[error("Entity with ID {0} not found")]
    EntityNotFound(u32),
}

#[inline]
pub fn canonicalize_zero(v: f32) -> f32 {
    let bits = v.to_bits();
    let sign = bits >> 31;
    let zero_mag = ((bits & 0x7FFF_FFFF) == 0) as u32;
    let is_neg_zero = sign & zero_mag;
    let mask = is_neg_zero.wrapping_neg();
    f32::from_bits(bits & !mask)
}

#[repr(C, align(32))]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct FighterState {
    pub position: Vector2D,     // Offset 0..8
    pub velocity: Vector2D,     // Offset 8..16
    pub health: f32,            // Offset 16..20
    pub stamina: f32,           // Offset 20..24
    pub id: u32,                // Offset 24..28
    pub team_id: u8,            // Offset 28..29
    pub facing_right: bool,     // Offset 29..30
    pub _pad: [u8; 2],          // Offset 30..32
}

impl FighterState {
    #[inline]
    pub fn canonicalize_mut(&mut self) {
        self.position.x = canonicalize_zero(self.position.x);
        self.position.y = canonicalize_zero(self.position.y);
        self.velocity.x = canonicalize_zero(self.velocity.x);
        self.velocity.y = canonicalize_zero(self.velocity.y);
        self.health = canonicalize_zero(self.health);
        self.stamina = canonicalize_zero(self.stamina);
    }

    /// Canonicalizing setter for position. Enforces positive zero (+0.0) invariant.
    #[inline]
    pub fn set_position(&mut self, x: f32, y: f32) {
        self.position.x = canonicalize_zero(x);
        self.position.y = canonicalize_zero(y);
    }

    /// Canonicalizing setter for velocity. Enforces positive zero (+0.0) invariant.
    #[inline]
    pub fn set_velocity(&mut self, x: f32, y: f32) {
        self.velocity.x = canonicalize_zero(x);
        self.velocity.y = canonicalize_zero(y);
    }

    /// Canonicalizing setter for health. Enforces positive zero (+0.0) invariant.
    #[inline]
    pub fn set_health(&mut self, health: f32) {
        self.health = canonicalize_zero(health);
    }

    /// Canonicalizing setter for stamina. Enforces positive zero (+0.0) invariant.
    #[inline]
    pub fn set_stamina(&mut self, stamina: f32) {
        self.stamina = canonicalize_zero(stamina);
    }
}

#[repr(C, align(64))]
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct WorldState {
    pub fighters: [FighterState; MAX_SIMULTANEOUS_ENTITIES], // 64 * 32 = 2048 bytes (Offset 0..2048)
    pub rng_state: PrngStatePOD,                             // 32 bytes (Offset 2048..2080)
    pub current_tick: u64,                                   // 8 bytes (Offset 2080..2088)
    pub active_count: u8,                                    // 1 byte (Offset 2088..2089)
    pub _header_pad: [u8; 23],                               // 23 bytes (Offset 2089..2112)
}

impl Default for WorldState {
    fn default() -> Self {
        Self {
            fighters: [FighterState::default(); MAX_SIMULTANEOUS_ENTITIES],
            rng_state: PrngStatePOD::default(),
            current_tick: 0,
            active_count: 0,
            _header_pad: [0; 23],
        }
    }
}

#[repr(C, align(32))]
struct Align32Marker;

#[repr(C, align(64))]
struct Align64Marker;

assert_eq_size!(FighterState, [u8; 32]);
assert_eq_align!(FighterState, Align32Marker);

assert_eq_size!(WorldState, [u8; 2112]);
assert_eq_align!(WorldState, Align64Marker);

impl WorldState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn get_fighter(&self, id: u32) -> Result<&FighterState, StateError> {
        for i in 0..(self.active_count as usize) {
            if self.fighters[i].id == id {
                return Ok(&self.fighters[i]);
            }
        }
        Err(StateError::EntityNotFound(id))
    }

    pub fn get_fighter_mut(&mut self, id: u32) -> Result<&mut FighterState, StateError> {
        for i in 0..(self.active_count as usize) {
            if self.fighters[i].id == id {
                return Ok(&mut self.fighters[i]);
            }
        }
        Err(StateError::EntityNotFound(id))
    }

    pub fn canonicalize_all(&mut self) {
        for i in 0..(self.active_count as usize) {
            self.fighters[i].canonicalize_mut();
        }
    }

    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        unsafe {
            core::slice::from_raw_parts(
                self as *const Self as *const u8,
                core::mem::size_of::<Self>(),
            )
        }
    }

    pub fn compute_hash(&self) -> u64 {
        xxh3_64(self.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_sizes() {
        assert_eq!(core::mem::size_of::<FighterState>(), 32);
        assert_eq!(core::mem::align_of::<FighterState>(), 32);
        assert_eq!(core::mem::size_of::<WorldState>(), 2112);
        assert_eq!(core::mem::align_of::<WorldState>(), 64);
    }

    #[test]
    fn test_xxh3_golden_vector() {
        // XXH3-64 golden reference vector for empty input (seed 0)
        assert_eq!(xxh3_64(b""), 0x2D06_8005_38D3_94C2);
    }

    #[test]
    fn test_setters_canonicalize_zero() {
        let mut fighter = FighterState::default();
        fighter.set_velocity(-0.0, -0.0);
        fighter.set_position(-0.0, 0.0);
        fighter.set_health(-0.0);
        fighter.set_stamina(-0.0);

        assert_eq!(fighter.velocity.x.to_bits(), 0x0000_0000);
        assert_eq!(fighter.velocity.y.to_bits(), 0x0000_0000);
        assert_eq!(fighter.position.x.to_bits(), 0x0000_0000);
        assert_eq!(fighter.position.y.to_bits(), 0x0000_0000);
        assert_eq!(fighter.health.to_bits(), 0x0000_0000);
        assert_eq!(fighter.stamina.to_bits(), 0x0000_0000);
    }

    #[test]
    fn test_state_hash_consistency() {
        let mut state_a = WorldState::new();
        state_a.active_count = 2;
        state_a.fighters[0].set_position(10.0, 5.0);
        state_a.fighters[0].set_velocity(1.0, 0.0);
        state_a.fighters[0].set_health(100.0);
        state_a.fighters[0].set_stamina(50.0);
        state_a.fighters[0].id = 1;
        state_a.fighters[0].team_id = 0;
        state_a.fighters[0].facing_right = true;

        state_a.fighters[1].set_position(20.0, 5.0);
        state_a.fighters[1].set_velocity(-1.0, 0.0);
        state_a.fighters[1].set_health(80.0);
        state_a.fighters[1].set_stamina(40.0);
        state_a.fighters[1].id = 2;
        state_a.fighters[1].team_id = 1;
        state_a.fighters[1].facing_right = false;
        state_a.current_tick = 42;

        let mut state_b = WorldState::new();
        state_b.active_count = 2;
        state_b.fighters[0].set_position(10.0, 5.0);
        state_b.fighters[0].set_velocity(1.0, 0.0);
        state_b.fighters[0].set_health(100.0);
        state_b.fighters[0].set_stamina(50.0);
        state_b.fighters[0].id = 1;
        state_b.fighters[0].team_id = 0;
        state_b.fighters[0].facing_right = true;

        state_b.fighters[1].set_position(20.0, 5.0);
        state_b.fighters[1].set_velocity(-1.0, 0.0);
        state_b.fighters[1].set_health(80.0);
        state_b.fighters[1].set_stamina(40.0);
        state_b.fighters[1].id = 2;
        state_b.fighters[1].team_id = 1;
        state_b.fighters[1].facing_right = false;
        state_b.current_tick = 42;

        assert_eq!(state_a.compute_hash(), state_b.compute_hash());
    }

    #[test]
    fn test_negative_zero_hash_invariance() {
        let mut state_pos = WorldState::new();
        state_pos.active_count = 1;
        state_pos.fighters[0].set_position(0.0, 0.0);
        state_pos.fighters[0].set_velocity(0.0, 0.0);
        state_pos.fighters[0].set_health(100.0);
        state_pos.fighters[0].set_stamina(50.0);
        state_pos.fighters[0].id = 1;
        state_pos.fighters[0].team_id = 0;
        state_pos.fighters[0].facing_right = true;

        let mut state_neg = WorldState::new();
        state_neg.active_count = 1;
        // Construct with raw -0.0 bits bypassing setters to test canonicalization gate
        state_neg.fighters[0] = FighterState {
            position: Vector2D::new(f32::from_bits(0x8000_0000), f32::from_bits(0x8000_0000)),
            velocity: Vector2D::new(f32::from_bits(0x8000_0000), f32::from_bits(0x8000_0000)),
            health: 100.0,
            stamina: 50.0,
            id: 1,
            team_id: 0,
            facing_right: true,
            _pad: [0; 2],
        };

        assert_ne!(state_pos.compute_hash(), state_neg.compute_hash());

        state_neg.canonicalize_all();
        assert_eq!(state_pos.compute_hash(), state_neg.compute_hash());
    }

    #[test]
    fn test_world_state_zero_alloc() {
        let mut state = WorldState::new();
        state.active_count = 1;
        state.fighters[0].id = 10;

        alloc_counter::deny_alloc(|| {
            let _cloned = state;
        });

        alloc_counter::deny_alloc(|| {
            let _hash = state.compute_hash();
        });

        alloc_counter::deny_alloc(|| {
            state.canonicalize_all();
        });

        alloc_counter::deny_alloc(|| {
            state.reset();
        });
    }

    #[test]
    fn test_entity_not_found() {
        let mut state = WorldState::new();
        state.active_count = 1;
        state.fighters[0].id = 5;

        assert!(state.get_fighter(5).is_ok());
        assert!(matches!(
            state.get_fighter(99),
            Err(StateError::EntityNotFound(99))
        ));
    }

    #[test]
    fn test_reset_zeroes_inactive_slots() {
        let mut state = WorldState::new();
        state.active_count = 5;
        state.fighters[4].id = 999;
        state.rng_state.seed = 12345;
        state.current_tick = 100;

        state.reset();

        assert_eq!(state.active_count, 0);
        assert_eq!(state.current_tick, 0);
        assert_eq!(state.rng_state.seed, 0);
        for i in 0..MAX_SIMULTANEOUS_ENTITIES {
            let fighter_bytes = unsafe {
                core::slice::from_raw_parts(
                    &state.fighters[i] as *const FighterState as *const u8,
                    core::mem::size_of::<FighterState>(),
                )
            };
            assert!(fighter_bytes.iter().all(|&b| b == 0));
        }
    }
}
