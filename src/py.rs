use crate::*;
use pyo3::{exceptions::*, prelude::*};
use std::collections::BTreeMap;

impl From<FarcError> for PyErr {
	fn from(value: FarcError) -> Self {
		match value {
			FarcError::Io(io_err) => PyErr::new::<PyIOError, _>(io_err.to_string()),
			FarcError::BinRead(bin_err) => PyErr::new::<PyException, _>(format!("{}", bin_err)),
			FarcError::NulError(_) => PyErr::new::<PyException, _>("Null in middle of entry name"),
			FarcError::MissingData => PyErr::new::<PyException, _>("File in header does not exist"),
		}
	}
}

#[pyfunction]
fn read(path: &str) -> PyResult<BTreeMap<String, Vec<u8>>> {
	Ok(Farc::read(path).map(|farc| farc.entries)?)
}

#[pyfunction]
#[pyo3(signature = (entries, path, compress=true))]
fn save(entries: BTreeMap<String, Vec<u8>>, path: &str, compress: bool) -> PyResult<()> {
	let farc = Farc { entries };
	farc.write(path, compress)?;
	Ok(())
}

#[pymodule]
fn farc(_: Python<'_>, m: &PyModule) -> PyResult<()> {
	m.add_function(wrap_pyfunction!(read, m)?)?;
	m.add_function(wrap_pyfunction!(save, m)?)?;

	Ok(())
}
