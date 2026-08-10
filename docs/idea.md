# Project: 2D Multi-Agent Tactical Combat Simulation Engine ($N$ vs $M$) Based on Frame-Data

## Selected Tech Stack

- **Primary Language (Core Engine & Simulation):** Rust (2021/2025 Edition)
- **Graphics & 2D Rendering Layer:** Raylib (`raylib` crate v6.0+)
- **Debugging Graphical User Interface (GUI & Overlay):** `egui` (via `egui-raylib` / `raylib-egui`)
- **AI / MARL Ecosystem:** Python 3.13+ (`torch`, `pettingzoo`, `gymnasium`, `numpy`)
- **Rust <-> Python Bindings:** PyO3 (v0.22+)
- **Python Extension Build Tool / Compiler:** Maturin (v1.5+)
- **Data & Configuration Management (Data-Driven):** `serde` + `serde_json`
- **Encapsulated Deterministic PRNG:** `rand` + `rand_chacha` (`ChaCha8Rng`)

## Dual Execution Architecture

```text
               ┌────────────────────────────────────────────────────────┐
               │            Deterministic Core Engine (Rust)            │
               │   - Frame-Data FSM        - Physics & Collisions       │
               │   - Seeded PRNG (ChaCha8) - JSON Configs & Weapons     │
               └───────────────────────────┬────────────────────────────┘
                                           │
                ┌──────────────────────────┴──────────────────────────┐
                ▼                                                     ▼
┌───────────────────────────────┐                     ┌───────────────────────────────┐
│      Headless Mode (PyO3)     │                     │       Visual & GUI Mode       │
│      (Python Extension)       │                     │  (Raylib + egui Integration)  │
│                               │                     │                               │
│ - PettingZoo / Gymnasium API  │                     │ - Decoupled 2D Rendering      │
│ - Zero GPU/UI overhead        │                     │ - Hitstop & Screen Shake      │
│ - Ultra-high SPS for MARL     │                     │ - Hitbox/Hurtbox Overlay      │
│ - Zero-Copy arrays to PyTorch │                     │ - Interactive Control Panel   │
└───────────────────────────────┘                     └───────────────────────────────┘

```

## 1. General Overview

The project consists of the conception, design, and implementation of a 2D multi-agent tactical combat simulation environment ($N$ vs $M$, with $N, M \ge 1$) driven by discrete events (*ticks*). The system abstracts combat through the rigorous control of time and space intervals (*Frame-Data*), emulating the depth of competitive fighting games where each action is divided into preparation (*Startup*), impact (*Active*), and recovery (*Recovery*) phases.

The main purpose of the project is to provide a high-performance, deterministic platform that serves as an experimentation environment for Multi-Agent Reinforcement Learning (MARL). The system allows modeling from symmetric 1v1 duels to asymmetric team combats (e.g., 1v2, 2v2, 3v2), where agents must dynamically learn distance management, timing, error reading, and tactical coordination.

A fundamental requirement of the project is the **generalization of behavior in response to equipment and stage variability**: agents must adapt their policies to at least 4 distinct weapon types. Furthermore, the architecture must allow the manual modification of the weapon assigned to any agent during the course of a training/simulation session. To facilitate human inspection and analysis, the engine includes a visual layer with advanced graphical feedback (*Juice*), strictly decoupled from the internal simulation loop.

---

## 2. Technology Selection Criteria (Technology Agnostic)

The choice of programming language, libraries, and tools is free and left to the developer's discretion, provided that the solution meets the following technical capability criteria:

- **Simulation Engine:** Must allow the execution of a discrete tick-based simulation loop, supporting high-speed execution in "headless" mode.
- **Reinforcement Learning Interface:** Must provide a multi-agent environment API that exposes standardized methods for stepping ticks, resetting scenarios, querying states/observations, delivering rewards, and termination signals.
- **Graphics / Rendering Layer:** Must be capable of rendering 2D geometric primitives, manipulating camera position, and managing temporary visual effects without blocking or desynchronizing the tick rate of the internal logic.
- **Configuration Management:** Must use structured, code-independent data formats to parameterize weapons, stages/scenarios, and fighter attributes.

---

## 3. Functional Requirements: Minimum Viable Product (MVP)

- **RF-01 (Multi-Agent Simulation $N$ vs $M$):** The environment must allow configuring and simulating combats between two teams with $N$ fighters in Team A and $M$ fighters in Team B ($N, M \ge 1$), concurrently managing the states of each entity in every tick.
- **RF-02 (Frame-Data Based Combat Logic):** Each executable action (attack, block, dodge) must be processed by a Finite State Machine that mandatorily transitions through three discrete temporal phases:
  - *Startup:* Preparation ticks without damage generation.
  - *Active:* Ticks where impact zones (*Hitboxes*) are enabled.
  - *Recovery:* Final ticks of vulnerability and immobility after executing the action.
- **RF-03 (2D Collision and Impact Detection):** The engine must calculate the geometric intersection of collision boxes (*Hitboxes*) with hurt boxes (*Hurtboxes*), resolving damage, stun, and knockback.
- **RF-04 (Catalog of 4 Unique Weapons):** Implement at least 4 weapon types with parameterized and differentiated attributes (duration of each frame-data phase, spatial reach, impact area, base damage, and stamina consumption).
- **RF-05 (Weapon Swapping Between Episodes/Matches):** Expose a mechanism/API within the initialization/reset cycle (`reset`) that allows dynamically reassigning the equipped weapon for any agent between matches or training sessions, enabling equipment variation without needing to recreate internal structures or recompute the environment.
- **RF-06 (Generalizable Observation Space):** The observation vector/dictionary provided to each agent must explicitly encode the numerical and temporal parameters of its current weapon at the current tick, allowing the model to condition its decisions on the weapon's characteristics.
- **RF-07 (Support for Multiple Stages/Scenarios):** The system must include at least 2 configurable scenarios/arenas that differ in spatial boundaries, platform dimensions, and spawn points.
- **RF-08 (Integrated Visual Feedback - "Juice"):** The visualization layer must mandatorily include:
  - *Hitstop:* Temporary pause of $k$ frames in the visual representation upon connecting a successful hit.
  - *Screen Shake:* Random camera offset proportional to the magnitude of the impact.
  - *Debugging Overlay:* Optional visual display of collision boxes color-coded according to the phase of the action.
- **RF-09 (Dual Execution Mode - Headless / Visual):** The system must be able to run in *headless* mode (without a graphic window) at maximum CPU speed for accelerated training, and in rendered mode at a constant refresh rate for inspection.

---

## 4. Non-Functional Requirements and Architectural Principles

### Non-Functional Requirements
- **RNF-01 (Headless Simulation Performance):** The engine must be capable of processing the simulation at high speed (a quantifiable minimum of global simulation steps per second in 2v2 engagements) so as not to become a bottleneck during AI training.
- **RNF-02 (Strict Determinism):** Given the same random seed (*seed*) and the same sequence of actions, the engine must produce exactly the same trajectory of states and numerical results.
- **RNF-03 (Logical-Visual Decoupling):** The effects of the graphical layer (graphical *Hitstop*, screen shakes, rendering) MUST NOT alter internal timing, state logic, impact detection, or reward calculation of the simulation.
- **RNF-04 (Data-Driven Design):** Weapon attributes, scenario boundaries, and physical characteristics of characters must be loaded from external configuration files without requiring recompilation or modification of the source code.

### Recommended Architectural Principles
- **Separation of Concerns:** Completely isolate the simulation engine (math and game rules), the reinforcement learning interface (transformation of states into observations/rewards), and the presentation layer (visual rendering).
- **Flexible Entity Management:** Design fighter entities in a modular fashion so that their properties (weapon, stamina attributes, team) can mutate at runtime without altering the agent's lifecycle in the simulation.

---

## 5. Room for Creativity and Proposed Improvements

This section presents computer science and engineering challenges where the student can demonstrate technical excellence:

- **Algorithmics and Generalization Challenge (Zero-Shot Evaluation with Unseen Weapons):**  
  Design an experimental regime where agents are trained using only 3 of the available weapons. Post-training, evaluate the agent's ability to infer efficient strategies when facing the 4th weapon (never seen during training), relying exclusively on reading the weapon parameters injected into its observation vector.

- **Simulation and Coordination Challenge (Tactical Emergence in $N$ vs $M$):**  
  Implement optional dynamics such as friendly fire (*Friendly Fire*), body-blocking mechanics between allies, or rewards for coverage and interrupting enemy attacks, analyzing the emergence of complex cooperative behaviors (e.g., distraction roles, flanking, or spatial coverage).

- **Interaction and Tooling Challenge (Real-Time Control and Inspection Panel):**  
  Develop an interactive user interface that allows a human operator to inspect the internal state of the simulation, modify an agent's weapon in real time using a graphical control, toggle between scenarios, or alter the tick speed during execution.

---

## 6. Success Criteria ("Definition of Done")

- [ ] **Multi-Agent API Compliance:** The environment correctly implements and validates a multi-agent API contract for $N$ vs $M$ scenarios.
- [ ] **Frame-Data and Collision Verification:** Actions correctly transition through the *Startup*, *Active*, and *Recovery* phases, and collisions are resolved deterministically according to the defined geometries.
- [ ] **Demonstrated Dynamic Weapon Swapping:** A script/test is included where an agent swaps weapons mid-simulation and the environment correctly updates its observation vector and combat logic seamlessly.
- [ ] **Rendering Independence:** Headless mode simulation is demonstrated to run significantly faster than visual mode, and graphical effects like *Hitstop* and *Screen Shake* do not affect internal temporal logic.
- [ ] **External Parameterization:** The catalog of at least 4 weapons and at least 2 scenarios is successfully loaded from independent configuration files.
- [ ] **Agent Validation:** Agents trained in the environment are shown to achieve superior performance compared to random-action agents in $N$ vs $M$ combat configurations.