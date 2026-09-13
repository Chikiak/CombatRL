## Issues for Phase 1: Deterministic Engine Core, Tick Loop, and Data Loading (Rust Core)

---

### ISSUE-005: [Engine Core] Data-Driven Arena and Fighter Attribute Configuration Schemas

**Metadata:** Layer: Backend / Core Rust  
**Direct Dependencies:** [ISSUE-001]

#### 1. Purpose and Rationale
* **Sequence:** First functional step of Phase 1. Establishes strongly-typed configuration contracts and parsing boundaries to load external arena and fighter attributes prior to simulation initialization.
* **Business Value:** Fulfills Data-Driven Architecture (NFR-04) and Multi-Scenario Support (FR-07) by enabling game designers and AI researchers to adjust arena boundaries, spawn coordinates, and entity baseline attributes via human-readable JSON files without code recompilation.
* **Prevented Technical Risks:** Prevents simulation panic states, floating-point microcode trap stalls (caused by `NaN`, infinite, or subnormal floats), zero-width bounding volumes, and out-of-bounds spawn placements through strict validation contracts during configuration ingest.
* **Architecture Decision:** Enforce a strict decoupling between the **Asset Ingestion DTO** (Serde-derived human-readable JSON schemas) and the **Compiled Runtime Attribute POD** (`repr(C, align(64))` fixed-width representation). Validate all spatial and numerical invariants during ingestion, converting variable string IDs into fixed zero-padded byte arrays (`[u8; 32]`) and storing compiled runtime configurations directly as inline value POD structs (`ArenaConfigPOD`, `FighterAttributesPOD`) aligned to L1 CPU cache lines (64 bytes) to eliminate pointer dereferencing and maximize data locality.

#### 2. Technical Specification and Contract
* **Paths / Components:**
    * `src/config.rs`
    * `assets/configs/arenas/default_arena.json`
    * `assets/configs/fighters/default_fighter.json`
* **Core Logic Specification and Structural Contracts:**
    * **Global System Capacity Constant:**
        - Declare `MAX_SIMULTANEOUS_ENTITIES: usize = 64` as the immutable, single source of truth constant in `src/config.rs`.
    * **Decoupled Architecture (Ingestion DTO vs Runtime Storage POD):**
        - External JSON MUST parse into intermediate Serde DTO structs.
        - Validated DTOs MUST compile into fixed-size runtime structs `ArenaConfigPOD` and `FighterAttributesPOD` declared as `repr(C, align(64))`.
    * **Exact In-Memory Struct Layout Specification (`ArenaConfigPOD`):**
        - Attributes declared strictly as `repr(C, align(64))` with an exact size of 64 bytes:
            1. `id: [u8; 32]` (Offset 0..32, 32 bytes): Fixed zero-padded byte array for arena profile identifier string.
            2. `width: f32` (Offset 32..36, 4 bytes): Arena horizontal width boundary.
            3. `height: f32` (Offset 36..40, 4 bytes): Arena vertical height boundary.
            4. `spawn_margin: f32` (Offset 40..44, 4 bytes): Minimum spatial clearance distance from outer walls for entity spawn coordinates.
            5. `_pad: [u8; 20]` (Offset 44..64, 20 bytes): Explicit zero-initialized padding array completing 64-byte L1 CPU cache line alignment.
    * **Exact In-Memory Struct Layout Specification (`FighterAttributesPOD`):**
        - Attributes declared strictly as `repr(C, align(64))` with an exact size of 64 bytes:
            1. `id: [u8; 32]` (Offset 0..32, 32 bytes): Fixed zero-padded byte array for fighter attribute profile identifier string.
            2. `max_health: f32` (Offset 32..36, 4 bytes): Maximum hit point capacity.
            3. `max_stamina: f32` (Offset 36..40, 4 bytes): Maximum stamina pool capacity.
            4. `move_speed: f32` (Offset 40..44, 4 bytes): Baseline kinematic velocity magnitude scalar.
            5. `weight: f32` (Offset 44..48, 4 bytes): Physical mass scalar used in knockback resistance calculation.
            6. `collider_size: Vector2D` (Offset 48..56, 8 bytes): Bounding box collision dimensions (`x: f32` 4 bytes, `y: f32` 4 bytes, 8-byte aligned).
            7. `_pad: [u8; 8]` (Offset 56..64, 8 bytes): Explicit zero-initialized padding array completing 64-byte L1 CPU cache line alignment.
    * **Spatial Vector Contract (`Vector2D`):**
        - Structured as an 8-byte C-compatible POD struct (`repr(C, align(8))`) containing `x: f32` and `y: f32`.
    * **Validation Boundaries and Numerical Invariants:**
        - **Subnormal and Non-Finite Rejection:** All floating-point fields MUST be strictly finite and normal numbers ($|x| \ge 1.17549435 \times 10^{-38}$ for non-zero values). Disallow `NaN`, `+Infinity`, `-Infinity`, and subnormal/denormal floats.
        - **Physical Bounds Constraints:**
            - Arena dimensions: `width >= 10.0`, `height >= 10.0`.
            - Fighter attributes: `max_health >= 1.0`, `max_stamina >= 1.0`, `move_speed >= 0.0`, `weight >= 0.1`.
            - Collider geometry: `collider_size.x >= 0.1`, `collider_size.y >= 0.1`.
            - Spawn Point Containment: All spawn coordinates MUST reside strictly within arena boundaries with collider margin clearance.
* **Expected Error Handling Contracts:**
    * Failures MUST return structured `Result<T, ConfigError>` using domain enum `ConfigError`:
        - `ConfigError::FileNotFound`
        - `ConfigError::InvalidSchema`
        - `ConfigError::InvalidBounds { parameter: &'static str, value: f32 }`
        - `ConfigError::SubnormalFloatDetected { parameter: &'static str }`

#### 3. Acceptance Criteria and Verification (DoD)
- [x] **Arena and Fighter JSON Schemas Created:** Populate default JSON files `assets/configs/arenas/default_arena.json` and `assets/configs/fighters/default_fighter.json` with valid attributes and physical colliders.
    * *Verification Command / Test:* `cargo check --no-default-features --features python`
- [x] **Successful Invariant Validation and Loading:** Validate via unit tests that loading valid DTOs produces compiled, validated `ArenaConfigPOD` and `FighterAttributesPOD` value structures with exact 64-byte alignment and zero dynamic allocations.
    * *Verification Command / Test:* `cargo test --lib config::tests::test_valid_loading`
- [x] **Rejection of Subnormal, Non-Finite, and Out-of-Bounds Configurations:** Verify that JSON inputs containing `NaN`, subnormal floats ($1.0 \times 10^{-40}$), negative boundaries, or invalid spawn points are rejected with appropriate `ConfigError` variants.
    * *Verification Command / Test:* `cargo test --lib config::tests::test_invalid_bounds_rejection`

---

### ISSUE-006: [Engine Core] Encapsulated Seeded PRNG and Deterministic RNG Manager

**Metadata:** Layer: Backend / Core Rust  
**Direct Dependencies:** [ISSUE-001]

#### 1. Purpose and Rationale
* **Sequence:** Built prior to world state and tick processing. Encapsulates all stochastic simulation decisions within a single, reproducible random generator.
* **Business Value:** Guarantees strict bit-exact determinism (NFR-02) across different operating systems and CPU architectures, ensuring experiment reproducibility in Multi-Agent Reinforcement Learning (MARL).
* **Prevented Technical Risks:** Prevents state loss during Serde serialization (resolving missing Serde traits in `ChaCha8Rng`), eliminates state drift during checkpoint re-hydration, and prohibits accidental calls to non-deterministic OS entropy sources.
* **Architecture Decision:** Encapsulate `rand_chacha::ChaCha8Rng` within a manager that serializes its exact state descriptor (`PrngStatePOD`). Provide zero-allocation state exports for hashing and implement deterministic stream hierarchy via `fork()`.

#### 2. Technical Specification and Contract
* **Paths / Components:**
    * `src/prng.rs`
* **Core Logic Specification and Structural Contracts:**
    * **Zero-Branch Generator Encapsulation (`DeterministicRng`):**
        - Encapsulate `ChaCha8Rng` directly as an active, unwrapped stack struct field to prevent branch mispredictions during sampling.
        - Public contract interfaces: `new(seed: u64) -> Self`, `gen_range_f32(low: f32, high: f32) -> f32`, `gen_bool(probability: f64) -> bool`, `fork() -> Self`, `export_pod() -> PrngStatePOD`.
    * **32-Byte Aligned State POD Descriptor (`PrngStatePOD`):**
        - Represent state via an exact 32-byte POD descriptor declared as `repr(C, align(32))`.
        - Internal field layout: `seed: u64` (8 bytes), `stream_id: u64` (8 bytes), `word_pos: u128` (16 bytes).
        - `export_pod()` MUST query `ChaCha8Rng::get_word_pos()` (`u128`) and `get_stream()` (`u64`) to capture complete internal cipher offsets without truncation.
    * **Deterministic Serde Hydration Contract:**
        - Bind serialization and deserialization directly to `PrngStatePOD`.
        - Re-hydration MUST reconstruct `ChaCha8Rng` from `seed`, re-applying `set_stream(stream_id)` and `set_word_pos(word_pos)` to restore the stream cursor identically.
    * **Division-Free IEEE 754 Mantissa Bit Injection Invariant:**
        - `gen_range_f32` MUST map uniformly generated integer bitfields onto normalized floating-point ranges $[low, high)$ via direct bitwise injection into the 23-bit IEEE 754 mantissa (masking bits to generate $[1.0, 2.0)$ and shifting/scaling using canonical float operations). Floating-point division (`/`) and dynamic FMA instructions are strictly prohibited.
    * **Deterministic Stream Hierarchy (`fork`):**
        - `fork()` MUST advance the parent PRNG state cursor deterministically prior to seed derivation, instantiating a child `DeterministicRng` with a non-overlapping stream sequence guarantee.
* **Expected Error Handling Contracts:**
    * Out-of-bounds or invalid state hydration descriptors MUST fail gracefully via `Result<DeterministicRng, PRNGError::InvalidState>` returning structured error enum `PRNGError::InvalidState`.

#### 3. Acceptance Criteria and Verification (DoD)
- [x] **Identical Sequence by Seed Test:** Verify that two independent `DeterministicRng` instances with identical seeds produce bit-identical floating-point sequences.
    * *Verification Command / Test:* `cargo test --lib prng::tests::test_identical_sequence`
- [x] **Serde Hydration State Restoration:** Verify that serializing a generator to JSON and deserializing it re-hydrates `word_pos` (`u128`) and `stream_id` (`u64`) correctly, producing identical subsequent values.
    * *Verification Command / Test:* `cargo test --lib prng::tests::test_serde_restoration`
- [x] **Zero Dynamic Allocation Verification:** Assert via unit tests that state export and uniform sampling execute strictly on the Stack with zero heap allocation.
    * *Verification Command / Test:* `cargo test --lib prng::tests::test_state_bytes_zero_alloc`

---

### ISSUE-007: [Engine Core] Core Simulation State Representation and Deterministic State Hashing

**Metadata:** Layer: Backend / Core Rust  
**Direct Dependencies:** [ISSUE-005, ISSUE-006]

#### 1. Purpose and Rationale
* **Sequence:** Establishes the core data container (`WorldState`) representing the complete physical world at any tick, enabling instant snapshotting and zero-allocation state hashing.
* **Business Value:** Delivers ultra-fast, bit-exact determinism validation via 64-bit `xxh3_64` state hashing, providing the foundational state contract for MARL state tracking and replay verification.
* **Prevented Technical Risks:** Prevents compiler padding gaps from leaking stack garbage into raw state hashes, avoids heap allocations during `step()` or snapshot copies (`Copy`/`Clone`), eliminates cache-line splitting across CPU cores, and prevents state hash divergences caused by IEEE 754 floating-point negative zero (`-0.0f32`) sign bit differences (`0x80000000` vs `0x00000000`).
* **Architecture Decision:** `FighterState` is a 32-byte POD struct (`repr(C, align(32))`). `WorldState` is aligned to L1 cache lines (`repr(C, align(64))`). Float canonicalization is enforced branchlessly during state updates, allowing `compute_hash()` to hash the raw byte slice `&[u8]` of `WorldState` in a single pass without dynamic memory allocation.

#### 2. Technical Specification and Contract
* **Paths / Components:**
    * `src/state.rs`
* **Core Logic Specification and Structural Contracts:**
    * **32-Byte SIMD-Aligned Entity Layout (`FighterState`):**
        - Declared as `repr(C, align(32))` with an exact size of 32 bytes.
        - Layout: `position` (Vector2D, 8 bytes), `velocity` (Vector2D, 8 bytes), `health` (f32, 4 bytes), `stamina` (f32, 4 bytes), `id` (u32, 4 bytes), `team_id` (u8, 1 byte), `facing_right` (bool, 1 byte), `_pad` ([u8; 2], explicit zeroed padding).
    * **64-Byte L1-Aligned Gapless Snapshot Layout (`WorldState`):**
        - Declared as `repr(C, align(64))` for contiguous stack allocation.
        - Layout:
            - `fighters`: Fixed contiguous array `[FighterState; MAX_SIMULTANEOUS_ENTITIES]` (64 * 32 = 2048 bytes).
            - `rng_state`: `PrngStatePOD` struct (32 bytes, `align(32)`).
            - `current_tick`: `u64` scalar (8 bytes).
            - `active_count`: `u8` scalar (1 byte).
            - `_header_pad`: `[u8; 23]` explicit zero-filled padding completing the 64-byte block boundary.
    * **Branchless Negative Zero (`-0.0f32`) Canonicalization Contract:**
        - Kinematic mutators MUST canonicalize floating-point zero values. Any floating-point field evaluating to `0.0f32` MUST have its sign bit stripped or forced to canonical positive zero (`+0.0f32`, bit pattern `0x00000000`) branchlessly prior to writing to `WorldState`, preventing bitwise hash divergences under `xxh3_64`.
    * **Zeroed Inactive Slot Invariant:**
        - Array slots in `fighters[active_count..MAX_SIMULTANEOUS_ENTITIES]` MUST be explicitly zero-filled (`0x00`) during initialization, `reset()`, or entity deactivation.
    * **Zero-Allocation Bitwise State Hashing (`compute_hash`):**
        - `compute_hash()` passes the raw byte slice `&[u8]` of `WorldState` to `xxh3_64` in a single pass, executing strictly on the Stack with zero heap allocation.
* **Expected Error Handling Contracts:**
    * Entity lookups using invalid entity IDs MUST return `Result<&FighterState, StateError::EntityNotFound>` using domain error enum `StateError::EntityNotFound`.

#### 3. Acceptance Criteria and Verification (DoD)
- [x] **Bit-Exact Hash Invariance Verification:** Validate that identical `WorldState` instances produce identical `xxh3_64` hashes across test executions.
    * *Verification Command / Test:* `cargo test --lib state::tests::test_state_hash_consistency`
- [x] **Float Negative Zero (`-0.0`) Canonical Hash Test:** Confirm that setting velocity or position components to `-0.0f32` yields the exact same hash as `+0.0f32`.
    * *Verification Command / Test:* `cargo test --lib state::tests::test_negative_zero_hash_invariance`
- [x] **Zero Heap Allocation Verification:** Assert via unit tests that copying, snapshotting, and hashing `WorldState` triggers zero dynamic memory allocations on the heap.
    * *Verification Command / Test:* `cargo test --lib state::tests::test_world_state_zero_alloc`

---

### ISSUE-008: [Engine Core] Fixed-Rate Discrete Tick Engine Loop and State Advance Mechanics

**Metadata:** Layer: Backend / Core Rust  
**Direct Dependencies:** [ISSUE-005, ISSUE-006, ISSUE-007]

#### 1. Purpose and Rationale
* **Sequence:** Integrates configuration data, PRNG streams, and state containers into the core simulation driver (`TickEngine`).
* **Business Value:** Provides the high-throughput discrete tick simulation loop ($\Delta t = 1/60\text{s} = 1\text{ tick}$), advancing kinematic state deterministically prior to Phase 2 combat rule integration.
* **Prevented Technical Risks:** Prevents heap allocations during tick steps, avoids SIMD vectorization breakages caused by branching logic, eliminates microcode traps from subnormal velocities, and prevents wall collision velocity sign contamination.
* **Architecture Decision:** `TickEngine` operates on a fixed-size stack array `[EntityActionPOD; MAX_SIMULTANEOUS_ENTITIES]` aligned to 32 bytes (`repr(C, align(32))`). `step()` advances state via a 3-phase SIMD-friendly sequential pipeline with Flush-To-Zero (FTZ) subnormal suppression and branchless spatial boundary clamping.

#### 2. Technical Specification and Contract
* **Paths / Components:**
    * `src/engine.rs`
* **Core Logic Specification and Structural Contracts:**
    * **Exact In-Memory Struct Layout Specification (`EntityActionPOD`):**
        - Attributes declared strictly as `repr(C, align(32))` with an exact size of 32 bytes:
             1. `move_intent: Vector2D` (Offset 0..8, 8 bytes): Normalized directional movement input vector (`x: f32` 4 bytes, `y: f32` 4 bytes, 8-byte aligned).
             2. `entity_id: u32` (Offset 8..12, 4 bytes): Target entity slot index within active simulation entities array.
             3. `action_flags: u32` (Offset 12..16, 4 bytes): Bitfield mask flags encoding discrete action triggers (Bit 0: Attack, Bit 1: Block, Bit 2: Dodge, Bit 3: Weapon Swap).
             4. `target_entity_id: u32` (Offset 16..20, 4 bytes): Slot index of targeted opposing entity for directional combat actions.
             5. `_pad: [u8; 12]` (Offset 20..32, 12 bytes): Explicit zero-initialized padding array completing 32-byte struct boundary alignment.
    * **Stack-Allocated Direct-Indexed Action Contract:**
        - Action inputs MUST be provided as a stack-allocated array `[EntityActionPOD; MAX_SIMULTANEOUS_ENTITIES]` declared as `repr(C, align(32))`.
        - Direct indexing by slot index ensures $O(1)$ constant memory access without dynamic heap allocations (`Vec`).
    * **Contiguous Attribute Storage Contract:**
        - Physical attributes MUST be stored in contiguous array `[FighterAttributesPOD; MAX_SIMULTANEOUS_ENTITIES]` aligned to 64-byte boundaries.
    * **3-Phase SIMD Execution Pipeline (`step`):**
        - **Phase 1 (Branchless Clamped Intent Normalization Pass):** Scan actions contiguously, computing direction vector norms. Clamp denominator via branchless `max(1.0)` and scale via IEEE division, ensuring zero division-by-zero or subnormal/NaN contamination without conditional branches.
        - **Phase 2 (Euler Kinematic Integration Pass with FTZ):** Update velocity ($\vec{v}_{t+1} = \vec{v}_t + \vec{a} \Delta t$) and position ($\vec{p}_{t+1} = \vec{p}_t + \vec{v}_{t+1} \Delta t$) contiguously. Apply Flush-To-Zero (FTZ): any velocity component $|v| < 1.17549435 \times 10^{-38}$ MUST be clamped to canonical positive zero (`+0.0f32`).
        - **Phase 3 (Branchless Boundary Clamping Pass):** Clamp positions strictly inside arena limits using branchless `min`/`max` instructions. Reset velocities on crossed axes strictly to `+0.0f32` using branchless conditional moves (`cmov`/select masks), preserving inward momentum and hash parity.
    * **PRNG and Tick Counter Advancement:**
        - Increment `WorldState.current_tick` by 1 and export active PRNG state bytes to `WorldState.rng_state` at the end of each tick step.
* **Expected Error Handling Contracts:**
    * Out-of-bounds entity action indices MUST return strongly-typed `Result<(), EngineError::InvalidEntityId>` using domain enum `EngineError::InvalidEntityId`.

#### 3. Acceptance Criteria and Verification (DoD)
- [x] **Discrete Kinematic Advance Verification:** Validate that calling `step()` with linear action intents advances fighter positions proportionally according to configured speed and increments tick count by 1.
    * *Verification Command / Test:* `cargo test --lib engine::tests::test_step_kinematics`
- [x] **Invalid Action Index Protection:** Confirm that passing actions with invalid `entity_id` values returns `Err(EngineError::InvalidEntityId)` without panicking.
    * *Verification Command / Test:* `cargo test --lib engine::tests::test_invalid_action_rejection`
- [x] **Arena Boundary Clamping and Canonical Velocity Verification:** Validate that entities colliding with arena boundaries are clamped geometrically and their velocity components on the collision axis are reset to `+0.0f32`.
    * *Verification Command / Test:* `cargo test --lib engine::tests::test_arena_bounds_clamping`

---

### ISSUE-009: [Engine Core] Bit-Exact Determinism Verification and Multi-Instance Regression Test Suite

**Metadata:** Layer: Backend / Core Rust  
**Direct Dependencies:** [ISSUE-008]

#### 1. Purpose and Rationale
* **Sequence:** Final validation gate for Phase 1. Certifies deterministic reproducibility and performance throughput across native Rust builds before proceeding to combat physics.
* **Business Value:** Formally guarantees bit-exact determinism (NFR-02) and verifies headless single-threaded simulation throughput > 1,000,000 SPS (NFR-01) for scalable MARL training.
* **Prevented Technical Risks:** Detects state hash divergences caused by FMA instruction rounding across architectures (x86_64 vs ARM64), prevents uninitialized stack padding leaks, and prevents Dead Code Elimination (DCE) in benchmarking harnesses.
* **Architecture Decision:** Implement cross-instance integration tests (`tests/test_determinism.rs`) verifying `xxh3_64` hash equality across 10,000 ticks, test pristine state isolation after `reset()`, and configure benchmarks in `benches/bench_engine.rs` utilizing `core::hint::black_box` barriers.

#### 2. Technical Specification and Contract
* **Paths / Components:**
    * `.cargo/config.toml`
    * `tests/test_determinism.rs`
    * `benches/bench_engine.rs`
* **Compiler Profile and Portability Specification (`.cargo/config.toml`):**
    - Declare rustflags: `rustflags = ["-C", "llvm-args=-enable-fma-lower=false", "-C", "target-feature=-fma"]` to disable FMA contraction globally across LLVM passes.
    - Enable LTO (`lto = "fat"`) and single codegen unit (`codegen-units = 1`) in `[profile.release]` for auto-vectorization.
* **Verification Suite Integration Protocol:**
    * Instantiate two independent simulation instances (`engine_a`, `engine_b`) with identical seeds (`0x5EED_CAFE_BABE_1234`).
    * Step both engines for 10,000 ticks using identical synthetic action sequences derived from an isolated PRNG stream.
    * Assert `engine_a.state().compute_hash() == engine_b.state().compute_hash()` at every single tick $k \in [1, 10000]$.
    * Validate `reset(new_seed)` isolation: Confirm that resetting an engine restores state hashes identically to a newly instantiated engine with that seed, asserting 100% zeroed bytes (`0x00`) in inactive entity slots `fighters[active_count..MAX_SIMULTANEOUS_ENTITIES]`.
* **Benchmarking Barrier Protocol (`benches/bench_engine.rs`):**
    * The benchmarking harness MUST pass all action input buffers and output state hashes through `core::hint::black_box()` to prevent LLVM Dead Code Elimination (DCE) from optimizing away execution ticks during throughput calculations.
* **Expected Error Handling Contracts:**
    * Determinism mismatches MUST panic immediately, outputting the exact tick index, divergent entity slot, and state hashes (`Engine A Hash: 0x..., Engine B Hash: 0x...`).

#### 3. Acceptance Criteria and Verification (DoD)
- [x] **10,000-Tick Determinism Verification:** Execute integration tests asserting 100% state hash equality over 10,000 ticks across independent engine instances.
    * *Verification Command / Test:* `cargo test --test test_determinism -- --nocapture`
- [x] **Reset Memory Residue and Isolation Verification:** Confirm that `reset()` restores pristine world states with zeroed inactive slots (`0x00`), matching freshly initialized engines identically.
    * *Verification Command / Test:* `cargo test --test test_determinism test_reset_isolation`
- [x] **Headless Simulation Throughput Certification (> 1,000,000 SPS):** Benchmark execution under `black_box` barriers certifying simulation throughput exceeds 1,000,000 Steps Per Second.
    * *Verification Command / Test:* `cargo bench`