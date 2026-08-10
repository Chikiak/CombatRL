## Issues for Phase 0: Project Initialization, Feature Flags, and Toolchain Setup

---

### ISSUE-001: Configure Rust Project Layout, Dual Targets, and Feature Flags
* **Sequence and Dependency:** This issue is the foundational step of the entire codebase. It must be implemented first because all core physics logic, PyO3 bindings, and Raylib/egui visual overlays rely on a defined Cargo crate structure with feature flags that isolate headless execution from graphical dependencies.
* **Description:** Initialize the Rust project structure supporting dual target outputs: a dynamic library (`cdylib` / `rlib`) for Python bindings (`src/lib.rs`) and an executable binary (`src/main.rs`) for visual interactive debugging. Configure Cargo feature flags to allow pure headless builds without linking graphical system libraries (such as X11, OpenGL, or GLFW).
* **Technical Specifications:**
    - **Directory Layout:**
      ```text
      .
      ├── Cargo.toml
      ├── pyproject.toml
      ├── src/
      │   ├── lib.rs
      │   └── main.rs
      └── tests/
      ```
    - **Cargo Targets (`Cargo.toml`):**
      ```toml
      [package]
      name = "marl_engine"
      version = "0.1.0"
      edition = "2021"
  
      [lib]
      name = "marl_engine_native"
      crate-type = ["cdylib", "rlib"]
      path = "src/lib.rs"
  
      [[bin]]
      name = "marl_engine_vis"
      path = "src/main.rs"
      required-features = ["visual"]
  
      [features]
      default = ["python"]
      python = ["dep:pyo3", "dep:numpy"]
      visual = ["dep:raylib", "dep:egui"]
  
      [dependencies]
      # Mandatory core simulation dependencies
      serde = { version = "1.0", features = ["derive"] }
      serde_json = "1.0"
      rand = "0.8"
      rand_chacha = "0.3"
      
      # Optional bindings (Python)
      pyo3 = { version = "0.22", features = ["extension-module"], optional = true }
      numpy = { version = "0.22", optional = true }
      
      # Optional visualization
      raylib = { version = "5.0", optional = true }
      egui = { version = "0.28", optional = true }
      ```
* **Constraints:** Headless compilation (`cargo check --no-default-features --features python`) must NOT link against any GPU, C/C++ rendering, or windowing system library (e.g., `libX11`, `libGL`, `libglfw`).
* **Acceptance Criteria:**
    - [ ] `Cargo.toml` is created with declared `[features]`, `[lib]`, `[[bin]]`, and explicit dependencies.
    - [ ] `src/lib.rs` and `src/main.rs` exist with basic compilation stubs (`fn main() {}`).
    - [ ] Execution of `cargo check --no-default-features --features python` compiles cleanly in a purely headless environment.
    - [ ] Execution of `cargo check --no-default-features --features visual` compiles the visual target.

---

### ISSUE-002: Setup Maturin Packaging and Python Environment Infrastructure
* **Sequence and Dependency:** Depends directly on `ISSUE-001`. Once the Rust crate and `cdylib` target are configured, this issue establishes the build bridge (`Maturin`) and standardizes the Python 3.13+ virtual environment with required RL dependencies (`torch`, `pettingzoo`, `gymnasium`, `numpy`).
* **Description:** Configure `pyproject.toml` to use Maturin as the build backend for compiling the Rust extension module into a Python wheel/package. Set up the Python virtual environment and dependency lock requirements.
* **Technical Specifications:**
    - **Configuration File (`pyproject.toml`):**
      ```toml
      [build-system]
      requires = ["maturin>=1.7,<2.0"]
      build-backend = "maturin"

      [project]
      name = "marl_engine"
      version = "0.1.0"
      description = "Deterministic 2D Multi-Agent Tactical Combat Simulation Engine"
      requires-python = ">=3.13"
      dependencies = [
      "numpy>=2.0.0",
      "gymnasium>=1.0.0",
      "pettingzoo>=1.25.0",
      "torch>=2.5.0"
      ]
      
      [tool.maturin]
      features = ["python"]
      features-default = false
      python-source = "python"
      module-name = "marl_engine._marl_engine_native"
      ```
    - **Python Package Wrapper (`python/marl_engine/__init__.py`):** Re-export native PyO3 modules cleanly.
* **Constraints:** Target Python environment must be 3.13+. Maturin must compile in `cdylib` mode generating CPython ABI3 compatible artifacts without requiring explicit visual feature flags during standard build processes.
* **Acceptance Criteria:**
    - [ ] `pyproject.toml` is created with Maturin set as the build-backend.
    - [ ] Python virtual environment (`.venv`) is initialized with `torch`, `pettingzoo`, `gymnasium`, and `numpy` installed.
    - [ ] `maturin develop --no-default-features --features python` executes successfully in the virtual environment and installs the package locally without compilation errors.

---

### ISSUE-003: Implement Initial PyO3 Native Module Interface and Zero-Copy Array Stub
* **Sequence and Dependency:** Depends on `ISSUE-001` (Rust crate layout) and `ISSUE-002` (Maturin setup). It implements the actual PyO3 module bindings in `src/lib.rs` to verify cross-language interoperability and memory sharing via `rust-numpy` before writing simulation logic.
* **Description:** Write the Rust implementation in `src/lib.rs` declaring a PyO3 native module `_marl_engine_native`. Implement a health check function `ping()` and a prototype zero-copy memory bridge function `create_dummy_obs()` using `rust-numpy`.
* **Technical Specifications:**
    - **Module Definition (`src/lib.rs`):**
      ```rust
      #[cfg(feature = "python")]
      use pyo3::prelude::*;
      #[cfg(feature = "python")]
      use numpy::PyArray1;
  
      #[cfg(feature = "python")]
      #[pyfunction]
      fn ping() -> &'static str {
          "marl_engine_core_ok"
      }
  
      #[cfg(feature = "python")]
      #[pyfunction]
      fn create_dummy_obs<'py>(py: Python<'py>, dim: usize) -> PyResult<Bound<'py, PyArray1<f32>>> {
          let vec: Vec<f32> = vec![0.0f32; dim];
          let array = PyArray1::from_vec_bound(py, vec);
          Ok(array)
      }
  
      #[cfg(feature = "python")]
      #[pymodule]
      fn _marl_engine_native(m: &Bound<'_, PyModule>) -> PyResult<()> {
          m.add_function(wrap_pyfunction!(ping, m)?)?;
          m.add_function(wrap_pyfunction!(create_dummy_obs, m)?)?;
          Ok(())
      }
      ```
* **Constraints:** PyO3 bindings must strictly use PyO3 v0.22+ `Bound<'py, T>` smart pointers (avoiding deprecated `PyResult<&PyArray1>`). Data transfer to Python must be zero-copy or direct transfer without secondary allocations where applicable.
* **Acceptance Criteria:**
    - [ ] `src/lib.rs` compiles without warnings under the `python` feature flag.
    - [ ] Importing `marl_engine._marl_engine_native` in Python exposes `ping()` and `create_dummy_obs()`.
    - [ ] Calling `ping()` from Python returns string `"marl_engine_core_ok"`.
    - [ ] Calling `create_dummy_obs(100)` returns a `numpy.ndarray` of shape `(100,)` and type `float32`.

---

### ISSUE-004: Headless Execution Verification and Dynamic Linkage Validation Test
* **Sequence and Dependency:** Depends on `ISSUE-001`, `ISSUE-002`, and `ISSUE-003`. It serves as the final integration gate for Phase 0, validating that the build pipeline produces a pure headless dynamic library that executes correctly in non-GUI Python environments.
* **Description:** Create an automated integration test script in Python (`tests/test_headless_bootstrap.py`) that imports the built Rust extension, verifies function correctness, asserts memory layout contiguous properties, and verifies that graphics libraries are not linked.
* **Technical Specifications:**
    - **Test Script (`tests/test_headless_bootstrap.py`):**
      ```python
      import unittest
      import numpy as np
      import marl_engine._marl_engine_native as engine
  
      class TestHeadlessBootstrap(unittest.TestCase):
          def test_engine_ping(self):
              res = engine.ping()
              self.assertEqual(res, "marl_engine_core_ok")
  
          def test_zero_copy_array_stub(self):
              dim = 256
              arr = engine.create_dummy_obs(dim)
              self.assertIsInstance(arr, np.ndarray)
              self.assertEqual(arr.shape, (dim,))
              self.assertEqual(arr.dtype, np.float32)
              self.assertTrue(arr.flags['C_CONTIGUOUS'])
  
      if __name__ == "__main__":
          unittest.main()
      ```
* **Constraints:** The test suite must run inside a headless environment (with `DISPLAY` unset or in a standard CI container without libgl1/x11 installed). Execution time must be < 1.0 second.
* **Acceptance Criteria:**
    - [ ] Running `maturin develop --no-default-features --features python` followed by `python -m unittest tests/test_headless_bootstrap.py` yields 100% passing tests.
    - [ ] Environment variable `DISPLAY=""` does not cause import failures or crashes.
    - [ ] Inspecting the output wheel or dynamic library (`.so`/`.dylib`/`.pyd`) confirms zero missing dynamically-linked GUI library dependencies.