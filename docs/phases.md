### Phase 0: Project Initialization, Feature Flags, and Toolchain Setup
* **Dependency Reasoning:** Establishes the repository scaffold, configures build targets, and decouples headless Python execution from graphical dependencies using Cargo feature flags before writing simulation logic.
* **Main Objective:** Create the project structure, configure `Cargo.toml` and `pyproject.toml`, establish Cargo feature flags, and validate initial Rust–Python interop via PyO3 and `rust-numpy`.
* **Requirements Covered:** Technical infrastructure foundation for all FRs/NFRs.
* **Key Deliverables:**
    * `Cargo.toml` configured with explicit feature flags:
      ```toml
      [features]
      default = ["python"]
      python = ["pyo3", "rust-numpy"]
      visual = ["raylib", "egui"] # raylib v5.5+ / sola-raylib v6.0+
      ```
    * `pyproject.toml` configured to use **Maturin** as the build backend for dynamic libraries (`cdylib`).
    * Python virtual environment (`venv`) with dependencies installed (`torch`, `pettingzoo`, `gymnasium`, `numpy`).
    * Dual-target codebase: `src/lib.rs` (PyO3 module for Python) and `src/main.rs` (standalone executable for visual mode).
* **Validation Criteria (Visible Progress):** Run `maturin develop --no-default-features --features python` inside a headless environment and execute a Python verification script that imports the module, calls `engine.ping()`, and creates a zero-copy NumPy array via `rust-numpy` without dynamic linking errors (e.g., missing X11/OpenGL/GLFW libraries).

---

### Phase 1: Deterministic Engine Core, Tick Loop, and Data Loading (Rust Core)
* **Dependency Reasoning:** The architectural foundation necessary to execute any simulation. Ensures strict seed-based determinism and Data-Driven infrastructure to parameterize entities and scenarios from external JSON files without altering code.
* **Main Objective:** Develop the discrete tick-based simulation loop in **Rust**, the entity structures, and the external configuration loader using **`serde_json`**.
* **Requirements Covered:** NFR-02, NFR-04, FR-07.
* **Key Deliverables:**
    * Discrete simulation loop driven by fixed-rate pulses (*ticks*).
    * External JSON loader (`serde` / `serde_json`) for arenas (spatial boundaries, spawn points) and fighter baseline attributes.
    * Seed-encapsulated Pseudo-Random Number Generator (PRNG) using `rand_chacha::ChaCha8Rng` for exact bit-level reproducibility across operating systems.
* **Validation Criteria (Visible Progress):** Run 10,000 ticks in headless console mode (`cargo test`) with two independent simulation instances sharing the same seed. Verify that state hashes are 100% identical at every tick.

---

### Phase 2: Combat Engine (Frame-Data FSM, Continuous Collisions, and Weapons)
* **Dependency Reasoning:** Integrates physics, temporal phases of Frame-Data, continuous collision checks to prevent tunneling, and weapon parameters. Exposing the re-assignment API on reset (`reset`) completes in-memory combat logic.
* **Main Objective:** Implement the action FSM (*Startup*, *Active*, *Recovery*), continuous 2D collision resolution (Swept AABB / Hitbox vs Hurtbox interpolation), and the catalog of 4 parameterized weapons in Rust.
* **Requirements Covered:** FR-02, FR-03, FR-04, FR-05.
* **Key Deliverables:**
    * Action FSM handling attack states, hitstun duration, blockstun, and knockback vector physics.
    * Continuous 2D collision detector to prevent fast-moving attacks from skipping hurtboxes (*tunneling*) between discrete ticks.
    * Deserialized catalog of 4 weapons, each projecting a **standardized, fixed-size numerical attribute vector** into the entity state.
    * Internal API method `reset(agent_weapon_map)` that reallocates weapon parameters dynamically per agent at episode boundaries.
* **Validation Criteria (Visible Progress):** Unit tests in Rust validating damage and knockback during the *Active* phase, verifying that high-speed attacks do not tunnel through hurtboxes, and confirming that `reset` updates equipment without changing memory layout or state vector sizes.

---

### Phase 3: Visual Layer (Raylib), Debug Overlay, and Interactive Control Panel (`egui`)
* **Dependency Reasoning:** Building the UI and visual layer immediately after combat logic provides the visual inspection tools needed to debug FSM phases, verify hitboxes, and manually test weapon swaps before training AI models.
* **Main Objective:** Implement decoupled 2D rendering using **Raylib** (compiled under `features = ["visual"]`), visual feedback (*Juice*), and an **`egui`** (or `raylib_imgui`) control panel.
* **Requirements Covered:** FR-08, FR-09, NFR-03, Interaction & Tooling Challenge.
* **Key Deliverables:**
    * Decoupled 2D renderer in `src/main.rs` that renders at variable frame rates without altering the internal logical tick rate.
    * Visual feedback subsystem: *Hitstop* (visual freeze frames) and *Screen Shake* (proportional offset).
    * *Debug Overlay* displaying color-coded hitboxes and hurtboxes based on the current FSM phase.
    * Interactive **`egui`** panel allowing live pause/resume, tick-by-tick stepping, real-time variable inspection, and dropdown menus to assign weapons for the next episode reset.
* **Validation Criteria (Visible Progress):** Run the standalone binary (`cargo run --features visual`), pause the execution via the `egui` overlay, select a different weapon for Agent 1, click reset, and step frame-by-frame to visually verify altered reach and frame-data phases in the color-coded debug view.

---

### Phase 4: Generalized MARL Interface ($N$ vs $M$) and Python Bindings (`PyO3` + `rust-numpy`)
* **Dependency Reasoning:** Connects the validated Rust engine to Python using `PyO3` and `rust-numpy`. Implementing the `PettingZoo ParallelEnv` API allows multi-agent reinforcement learning algorithms to interact with the environment at maximum throughput.
* **Main Objective:** Expose the engine to Python via **`PyO3`**, implementing a standardized multi-agent `ParallelEnv` interface (compatible with PettingZoo and Gymnasium) for $N$ vs $M$ scenarios.
* **Requirements Covered:** FR-01, FR-06.
* **Key Deliverables:**
    * Python module compiled via Maturin exposing `ParallelEnv` methods (`reset`, `step`, `state`).
    * **Fixed-size observation space encoder** using `rust-numpy` to pass contiguous array views directly to PyTorch tensors (zero-copy), explicitly injecting standardized numerical weapon parameters into each agent's observation vector.
    * Python abstraction layer for agent control (supporting dummy scripts, random policies, or PyTorch neural networks).
* **Validation Criteria (Visible Progress):** Execute a Python test script importing the compiled module (`import my_marl_engine`) running a 2v2 match with mixed random and passive policies without throwing exceptions, confirming observation dictionary shapes remain invariant regardless of equipped weapons.

---

### Phase 5: Curriculum Training Pipeline, Reward Shaping, and Self-Play (Python / PyTorch)
* **Dependency Reasoning:** Executes policy learning using Python training scripts and PyTorch without modifying the underlying Rust engine code.
* **Main Objective:** Implement a 3-stage training pipeline (Dummy, 1v1 Self-Play, and $N$ vs $M$ MAPPO) with shaped reward functions that enforce spacing, timing, and tactical awareness.
* **Requirements Covered:** FR-01, FR-05, FR-06.
* **Key Deliverables:**
    * **Reward Shaping Subsystem:** Rewards hit damage and knockback while penalizing stamina depletion and whiffed attacks (*whiff penalty*) during the *Recovery* phase to teach distance control (*spacing*).
    * **Stage A (Primitives):** Training script against static, moving, and attacking `DummyPolicy` targets.
    * **Stage B (1v1 Duels):** *Self-Play* pipeline with checkpoint matchmaking and random weapon rotation on each `reset()`.
    * **Stage C ($N$ vs $M$ Team Combat):** Multi-agent policy optimization (MAPPO/PPO) for 2v2 and asymmetric team configurations.
* **Validation Criteria (Visible Progress):** TensorBoard/WandB logs showing policy convergence across stages, demonstrating that trained agents achieve >85% win-rate against dummy baselines and maintain positive win-rates against past self-play checkpoints.

---

### Phase 6: Zero-Shot Generalization Experiment and Advanced Tactical Dynamics
* **Dependency Reasoning:** Uses trained multi-agent policies to evaluate generalization capabilities under unseen equipment and complex cooperative mechanics.
* **Main Objective:** Evaluate Zero-Shot generalization on a 4th weapon held out during Phase 5 training, and implement optional tactical team rules in Rust/Python.
* **Requirements Covered:** Zero-Shot Generalization Challenge, Tactical Emergence Challenge.
* **Key Deliverables:**
    * Optional engine ruleset for Friendly Fire, ally body-blocking, and attack interruption/peeling.
    * Zero-Shot evaluation script equipping agents with the unseen 4th weapon at episode reset.
    * Quantitative evaluation report assessing tactical emergence (e.g., flanking, covering allies, role specialization).
* **Validation Criteria (Visible Progress):** Quantitative data showing that agents immediately adapt their effective combat range when equipped with the unseen 4th weapon, relying exclusively on reading weapon parameters injected into their static observation vector.

---

### Phase 7: Vectorized Multi-Environment Headless Acceleration (Rayon), Benchmarks, and DoD Validation
* **Dependency Reasoning:** Maximizes execution speed in headless mode by vectorizing multiple environment instances across CPU threads using `Rayon`, concluding formal verification of all Definition of Done items.
* **Main Objective:** Implement batch multi-environment parallelization in Rust (`VectorizedEnv`), execute automated benchmark suites, and certify complete compliance with project requirements.
* **Requirements Covered:** NFR-01, Full Definition of Done.
* **Key Deliverables:**
    * `VectorizedEnv` module in Rust using **`Rayon`** to simulate $K$ parallel environment instances simultaneously (avoiding micro-threading overhead within a single $N$ vs $M$ match).
    * Automated regression and determinism test suite (`cargo bench` / `cargo test`).
    * Full Definition of Done compliance matrix and demonstration script.
* **Validation Criteria (Visible Progress):** Benchmark output certifying simulation throughput exceeding tens of thousands of Steps Per Second (SPS) in vectorized headless mode, alongside a 100% pass rate on the Definition of Done checklist.