use crate::config::{ArenaConfigPOD, FighterAttributesPOD, Vector2D, MAX_SIMULTANEOUS_ENTITIES};
use crate::prng::DeterministicRng;
use crate::state::{FighterState, WorldState};
use static_assertions::{assert_eq_align, assert_eq_size};
use thiserror::Error;

#[derive(Debug, PartialEq, Error)]
pub enum EngineError {
    #[error("Entity action references invalid slot index {0}")]
    InvalidEntityId(u32),
    #[error("Cannot spawn: maximum simultaneous entities reached")]
    MaxEntitiesReached,
}

#[repr(C, align(32))]
#[derive(Copy, Clone, Debug, Default)]
pub struct EntityActionPOD {
    pub move_intent: Vector2D,   // Offset 0..8
    pub entity_id: u32,          // Offset 8..12
    pub action_flags: u32,       // Offset 12..16
    pub target_entity_id: u32,   // Offset 16..20
    pub _pad: [u8; 12],          // Offset 20..32
}

#[repr(C, align(32))]
struct Align32Marker;

assert_eq_size!(EntityActionPOD, [u8; 32]);
assert_eq_align!(EntityActionPOD, Align32Marker);

pub const DT: f32 = 1.0 / 60.0;

#[inline(always)]
fn ftz_zero(v: f32) -> f32 {
    let bits = v.to_bits();
    let exp = (bits >> 23) & 0xFF;
    let flush = (exp == 0) as u32;
    f32::from_bits(bits & !(flush.wrapping_neg()))
}

pub struct TickEngine {
    arena: ArenaConfigPOD,
    template: FighterAttributesPOD,
    attributes: [FighterAttributesPOD; MAX_SIMULTANEOUS_ENTITIES],
    state: WorldState,
    rng: DeterministicRng,
}

impl TickEngine {
    pub fn new(
        seed: u64,
        arena: ArenaConfigPOD,
        template: FighterAttributesPOD,
    ) -> Result<Self, EngineError> {
        let mut engine = Self {
            arena,
            template,
            attributes: [FighterAttributesPOD::default(); MAX_SIMULTANEOUS_ENTITIES],
            state: WorldState::new(),
            rng: DeterministicRng::new(seed),
        };

        // Automatic symmetric spawn: Fighter 0 (left), Fighter 1 (right)
        let half_h = arena.height * 0.5;
        engine.spawn(Vector2D::new(arena.spawn_margin, half_h), 0)?;
        engine.spawn(Vector2D::new(arena.width - arena.spawn_margin, half_h), 1)?;

        // Set initial facing
        if engine.state.active_count >= 2 {
            engine.state.fighters[0].facing_right = true;
            engine.state.fighters[1].facing_right = false;
        }

        Ok(engine)
    }

    #[allow(clippy::field_reassign_with_default)]
    pub fn spawn(&mut self, position: Vector2D, team_id: u8) -> Result<u32, EngineError> {
        let slot = self.state.active_count as usize;
        if slot >= MAX_SIMULTANEOUS_ENTITIES {
            return Err(EngineError::MaxEntitiesReached);
        }

        let slot_u32 = slot as u32;
        self.attributes[slot] = self.template;

        let mut fighter = FighterState::default();
        fighter.id = slot_u32;
        fighter.team_id = team_id;
        fighter.set_position(position.x, position.y);
        fighter.set_velocity(0.0, 0.0);
        fighter.set_health(self.template.max_health);
        fighter.set_stamina(self.template.max_stamina);
        fighter.facing_right = team_id == 0;

        self.state.fighters[slot] = fighter;
        self.state.active_count += 1;

        Ok(slot_u32)
    }

    #[allow(clippy::needless_range_loop)]
    pub fn step(
        &mut self,
        actions: &[EntityActionPOD; MAX_SIMULTANEOUS_ENTITIES],
    ) -> Result<(), EngineError> {
        let active_count = self.state.active_count as usize;

        // Phase 0: Validation pass (fail-fast before any mutation)
        for i in 0..active_count {
            let act_id = actions[i].entity_id;
            if act_id as usize != i || act_id >= self.state.active_count as u32 {
                return Err(EngineError::InvalidEntityId(act_id));
            }
        }

        // Execute 3-phase pipeline for each active entity
        for i in 0..active_count {
            let act = &actions[i];
            let attrs = &self.attributes[i];
            let fighter = &mut self.state.fighters[i];

            // Phase 1: Branchless Clamped Intent Normalization (IEEE division with clamped divisor)
            let intent = act.move_intent;
            let len_sq = intent.x * intent.x + intent.y * intent.y;
            let abs_len = len_sq.sqrt();
            let abs_len_safe = abs_len.max(1.0); // branchless maxss
            let scale = 1.0 / abs_len_safe;      // divisor never 0 or <1

            let dir_x = intent.x * scale;
            let dir_y = intent.y * scale;

            // Phase 2: Euler Kinematic Integration with FTZ
            let target_vx = dir_x * attrs.move_speed;
            let target_vy = dir_y * attrs.move_speed;

            let vx = ftz_zero(target_vx);
            let vy = ftz_zero(target_vy);

            fighter.set_velocity(vx, vy);

            let new_x = ftz_zero(fighter.position.x + vx * DT);
            let new_y = ftz_zero(fighter.position.y + vy * DT);

            fighter.set_position(new_x, new_y);

            // Phase 3: Branchless Boundary Clamping
            let half_w = attrs.collider_size.x * 0.5;
            let half_h = attrs.collider_size.y * 0.5;

            let min_x = half_w;
            let max_x = self.arena.width - half_w;
            let min_y = half_h;
            let max_y = self.arena.height - half_h;

            let old_x = fighter.position.x;
            let old_y = fighter.position.y;

            let clamped_x = old_x.min(max_x).max(min_x);
            let clamped_y = old_y.min(max_y).max(min_y);

            let crossed_x = (clamped_x != old_x) as u32;
            let crossed_y = (clamped_y != old_y) as u32;

            let mask_x = crossed_x.wrapping_neg();
            let mask_y = crossed_y.wrapping_neg();

            let final_vx = f32::from_bits(fighter.velocity.x.to_bits() & !mask_x);
            let final_vy = f32::from_bits(fighter.velocity.y.to_bits() & !mask_y);

            fighter.set_position(clamped_x, clamped_y);
            fighter.set_velocity(final_vx, final_vy);
        }

        // Advance tick counter and export PRNG state
        self.state.current_tick += 1;
        self.state.rng_state = self.rng.export_pod();

        Ok(())
    }

    pub fn state(&self) -> &WorldState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut WorldState {
        &mut self.state
    }

    pub fn reset(&mut self, new_seed: u64) -> Result<(), EngineError> {
        let arena = self.arena;
        let template = self.template;
        *self = Self::new(new_seed, arena, template)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::loader::{load_default_arena, load_default_fighter};
    use crate::state::canonicalize_zero;

    #[test]
    fn test_entity_action_layout() {
        assert_eq!(std::mem::size_of::<EntityActionPOD>(), 32);
        assert_eq!(std::mem::align_of::<EntityActionPOD>(), 32);
    }

    #[test]
    fn test_ftz_subnormal_flush() {
        // Subnormal positive float
        let subnormal = f32::from_bits(0x0000_0001);
        assert_eq!(ftz_zero(subnormal), 0.0);
        assert_eq!(ftz_zero(0.0), 0.0);
        assert_eq!(ftz_zero(-0.0), 0.0);
        // Normal float
        let normal = 1.5_f32;
        assert_eq!(ftz_zero(normal), 1.5);
    }

    #[test]
    fn test_step_kinematics() {
        let arena = load_default_arena().unwrap();
        let fighter_attr = load_default_fighter().unwrap();
        let mut engine = TickEngine::new(42, arena, fighter_attr).unwrap();

        let initial_x = engine.state().fighters[0].position.x;
        let speed = fighter_attr.move_speed;

        let mut actions = [EntityActionPOD::default(); MAX_SIMULTANEOUS_ENTITIES];
        actions[0].entity_id = 0;
        actions[0].move_intent = Vector2D::new(1.0, 0.0);
        actions[1].entity_id = 1;
        actions[1].move_intent = Vector2D::new(0.0, 0.0);

        engine.step(&actions).unwrap();

        assert_eq!(engine.state().current_tick, 1);
        let expected_x = canonicalize_zero(initial_x + speed * DT);
        assert_eq!(engine.state().fighters[0].position.x, expected_x);
    }

    #[test]
    fn test_invalid_action_rejection() {
        let arena = load_default_arena().unwrap();
        let fighter_attr = load_default_fighter().unwrap();
        let mut engine = TickEngine::new(42, arena, fighter_attr).unwrap();

        let mut actions = [EntityActionPOD::default(); MAX_SIMULTANEOUS_ENTITIES];
        // Slot 0 action has wrong entity_id
        actions[0].entity_id = 99;

        let res = engine.step(&actions);
        assert!(matches!(res, Err(EngineError::InvalidEntityId(99))));
        assert_eq!(engine.state().current_tick, 0); // No state mutation on validation failure
    }

    #[test]
    fn test_arena_bounds_clamping() {
        let arena = load_default_arena().unwrap(); // width = 30.0
        let fighter_attr = load_default_fighter().unwrap(); // collider_size.x = 0.8 -> half_w = 0.4
        let mut engine = TickEngine::new(42, arena, fighter_attr).unwrap();

        // Teleport fighter 0 right near the right wall
        engine.state_mut().fighters[0].set_position(29.8, 9.0);

        let mut actions = [EntityActionPOD::default(); MAX_SIMULTANEOUS_ENTITIES];
        actions[0].entity_id = 0;
        actions[0].move_intent = Vector2D::new(1.0, 0.0);
        actions[1].entity_id = 1;
        actions[1].move_intent = Vector2D::new(0.0, 0.0); // push right into wall

        engine.step(&actions).unwrap();

        let max_x = arena.width - (fighter_attr.collider_size.x * 0.5);
        assert_eq!(engine.state().fighters[0].position.x, max_x);
        assert_eq!(engine.state().fighters[0].velocity.x.to_bits(), 0x0000_0000);
    }

    #[test]
    fn test_step_zero_alloc() {
        let arena = load_default_arena().unwrap();
        let fighter_attr = load_default_fighter().unwrap();
        let mut engine = TickEngine::new(42, arena, fighter_attr).unwrap();

        let mut actions = [EntityActionPOD::default(); MAX_SIMULTANEOUS_ENTITIES];
        actions[0].entity_id = 0;
        actions[0].move_intent = Vector2D::new(0.5, 0.5);
        actions[1].entity_id = 1;
        actions[1].move_intent = Vector2D::new(-0.5, 0.0);

        alloc_counter::deny_alloc(|| {
            engine.step(&actions).unwrap();
        });
    }

    #[test]
    fn test_determinism_seed() {
        let arena = load_default_arena().unwrap();
        let fighter_attr = load_default_fighter().unwrap();

        let mut engine_a = TickEngine::new(0x5EED_CAFE, arena, fighter_attr).unwrap();
        let mut engine_b = TickEngine::new(0x5EED_CAFE, arena, fighter_attr).unwrap();

        let mut actions = [EntityActionPOD::default(); MAX_SIMULTANEOUS_ENTITIES];
        actions[0].entity_id = 0;
        actions[0].move_intent = Vector2D::new(0.707, 0.707);
        actions[1].entity_id = 1;
        actions[1].move_intent = Vector2D::new(-1.0, 0.0);

        for _ in 0..100 {
            engine_a.step(&actions).unwrap();
            engine_b.step(&actions).unwrap();
            assert_eq!(engine_a.state().compute_hash(), engine_b.state().compute_hash());
        }
    }

    #[test]
    fn test_engine_reset() {
        let arena = load_default_arena().unwrap();
        let fighter_attr = load_default_fighter().unwrap();
        let mut engine = TickEngine::new(42, arena, fighter_attr).unwrap();

        engine.reset(123).unwrap();
        assert_eq!(engine.state().current_tick, 0);
    }
}
