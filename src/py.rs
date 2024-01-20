use crate::*;
use pyo3::{exceptions::*, prelude::*};
use std::collections::BTreeMap;

impl From<FarcError> for PyErr {
	fn from(value: FarcError) -> Self {
		match value {
			FarcError::BinaryParserError(binary_err) => {
				PyErr::new::<PyIOError, _>(binary_err.to_string())
			}
			FarcError::Unsupported => PyErr::new::<PyException, _>("Unsupported file"),
			FarcError::IoError(io_err) => PyErr::new::<PyException, _>(io_err.to_string()),
			FarcError::PendingWrites => PyErr::new::<PyException, _>(
				"Cannot save file please call finish_writes on any entries that have been modified",
			),
		}
	}
}

#[pyfunction]
fn read(path: String) -> PyResult<BTreeMap<String, Vec<u8>>> {
	Ok(Farc::from_file(path).map(|farc| {
		farc.entries
			.into_iter()
			.map(|(name, entry)| (name, entry.to_buf_const().unwrap().clone()))
			.collect()
	})?)
}

#[pyfunction]
#[pyo3(signature = (entries, path, compress=true))]
fn save(entries: BTreeMap<String, Vec<u8>>, path: String, compress: bool) -> PyResult<()> {
	let farc = Farc {
		entries: entries
			.into_iter()
			.map(|(name, entry)| (name, BinaryParser::from_buf(entry)))
			.collect(),
	};
	farc.write_file(&path, compress)?;
	Ok(())
}

#[pymodule]
fn farc(_: Python<'_>, m: &PyModule) -> PyResult<()> {
	m.add_function(wrap_pyfunction!(read, m)?)?;
	m.add_function(wrap_pyfunction!(save, m)?)?;

	Ok(())
}
