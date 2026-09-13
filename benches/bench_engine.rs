use marl_engine_native::config::MAX_SIMULTANEOUS_ENTITIES;
use marl_engine_native::engine::{EntityActionPOD, TickEngine};
use marl_engine_native::prng::DeterministicRng;
use marl_engine_native::{load_default_arena, load_default_fighter, Vector2D};
use std::hint::black_box;
use std::time::Instant;

fn main() {
    let arena = load_default_arena().unwrap();
    let template = load_default_fighter().unwrap();
    let mut engine = TickEngine::new(0x5EED_CAFE_BABE_1234, arena, template).unwrap();

    // Buffer de acciones sintético determinista (isolated stream, pre-generado una vez)
    let mut action_rng = DeterministicRng::new(0xA5A5_5A5A_DEAD_BEEF);
    let mut actions = [EntityActionPOD::default(); MAX_SIMULTANEOUS_ENTITIES];
    for (i, action) in actions.iter_mut().enumerate().take(engine.state().active_count as usize) {
        action.entity_id = i as u32;
        action.move_intent = Vector2D::new(action_rng.gen_range_f32(-1.0, 1.0), action_rng.gen_range_f32(-1.0, 1.0));
    }

    // Warmup
    for _ in 0..10_000 { black_box(engine.step(black_box(&actions))).unwrap(); }

    // Medición
    const N: usize = 1_000_000;
    let start = Instant::now();
    let mut acc: u64 = 0;
    for _ in 0..N {
        black_box(engine.step(black_box(&actions))).unwrap();
        acc ^= black_box(engine.state().compute_hash());
    }
    let elapsed = start.elapsed();
    let sps = N as f64 / elapsed.as_secs_f64();
    black_box(acc); // evita DCE sobre el resultado acumulado

    // OPCIÓN C: certificación condicional por variable de entorno
    if std::env::var("BENCH_STRICT_CERTIFY").is_ok() {
        assert!(sps > 1_000_000.0, "throughput below 1M SPS: {sps}");
    } else {
        eprintln!("Steps Per Second: {:.0}", sps);
    }
}
