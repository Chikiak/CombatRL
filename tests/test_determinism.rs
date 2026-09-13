use marl_engine_native::config::MAX_SIMULTANEOUS_ENTITIES;
use marl_engine_native::engine::{EntityActionPOD, TickEngine};
use marl_engine_native::prng::DeterministicRng;
use marl_engine_native::{load_default_arena, load_default_fighter, Vector2D};

const SEED: u64 = 0x5EED_CAFE_BABE_1234;
const ACTION_SEED: u64 = 0xA5A5_5A5A_DEAD_BEEF;

#[test]
fn test_determinism_10000_ticks() {
    let arena = load_default_arena().unwrap();
    let template = load_default_fighter().unwrap();
    let mut engine_a = TickEngine::new(SEED, arena, template).unwrap();
    let mut engine_b = TickEngine::new(SEED, arena, template).unwrap();

    let mut action_rng = DeterministicRng::new(ACTION_SEED);
    let mut actions = [EntityActionPOD::default(); MAX_SIMULTANEOUS_ENTITIES];

    for tick in 1..=10_000 {
        for (i, action) in actions.iter_mut().enumerate().take(2) {
            action.entity_id = i as u32;
            action.move_intent = Vector2D::new(
                action_rng.gen_range_f32(-1.0, 1.0),
                action_rng.gen_range_f32(-1.0, 1.0),
            );
        }

        engine_a.step(&actions).unwrap();
        engine_b.step(&actions).unwrap();

        let ha = engine_a.state().compute_hash();
        let hb = engine_b.state().compute_hash();

        if ha != hb {
            let active_count = engine_a.state().active_count as usize;
            let mut slot = 0;
            for (i, (fa, fb)) in engine_a
                .state()
                .fighters
                .iter()
                .zip(engine_b.state().fighters.iter())
                .enumerate()
                .take(active_count)
            {
                if fa != fb {
                    slot = i;
                    break;
                }
            }
            panic!("DIFF at tick {} slot {}: A={:#018x} B={:#018x}", tick, slot, ha, hb);
        }
    }
}

#[test]
fn test_reset_isolation() {
    let arena = load_default_arena().unwrap();
    let template = load_default_fighter().unwrap();
    let mut engine = TickEngine::new(SEED, arena, template).unwrap();

    let mut action_rng = DeterministicRng::new(0xB0B0_F00D_1234_5678);
    let mut actions = [EntityActionPOD::default(); MAX_SIMULTANEOUS_ENTITIES];
    for _ in 0..500 {
        for (i, action) in actions.iter_mut().enumerate().take(2) {
            action.entity_id = i as u32;
            action.move_intent = Vector2D::new(
                action_rng.gen_range_f32(-1.0, 1.0),
                action_rng.gen_range_f32(-1.0, 1.0),
            );
        }
        engine.step(&actions).unwrap();
    }

    const NEW_SEED: u64 = 0xDEAD_BEEF_0000_0001;
    engine.reset(NEW_SEED).unwrap();

    let fresh = TickEngine::new(NEW_SEED, arena, template).unwrap();

    assert_eq!(*engine.state(), *fresh.state());

    let active_count = engine.state().active_count as usize;
    for i in active_count..MAX_SIMULTANEOUS_ENTITIES {
        let bytes = unsafe {
            core::slice::from_raw_parts(
                &engine.state().fighters[i] as *const marl_engine_native::state::FighterState as *const u8,
                core::mem::size_of::<marl_engine_native::state::FighterState>(),
            )
        };
        assert!(bytes.iter().all(|&b| b == 0));
    }
}
