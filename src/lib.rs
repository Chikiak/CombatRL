#![cfg_attr(feature = "python", allow(clippy::useless_conversion))]

pub mod config;

#[cfg(test)]
mod test_alloc;

#[cfg(feature = "python")]
use numpy::PyArray1;
#[cfg(feature = "python")]
use pyo3::prelude::*;

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
