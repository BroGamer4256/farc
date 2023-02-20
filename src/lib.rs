#![allow(dead_code)]
use binrw::prelude::*;
use binrw::*;
use io::{Read, Seek, Write};
use std::collections::BTreeMap;

pub mod py;

#[derive(Debug, BinRead)]
#[br(big)]
enum FarcFileReader {
	#[br(magic = b"FArc")]
	Uncompressed(FarcHeaderReader),
	#[br(magic = b"FArC")]
	Compressed(CompressedFarcHeaderReader),
}

fn farc_header_reader_compressed<R: io::Read + io::Seek>(
	reader: &mut R,
	options: &ReadOptions,
	header_length: (u32,),
) -> Result<Vec<(NullString, u32, u32, u32)>, Error> {
	let mut remaining_header = header_length.0 - 4;
	let position = reader.stream_position()?;
	let mut out = vec![];
	while remaining_header > 0 {
		let name = reader.read_type::<NullString>(options.endian())?;
		let offset = reader.read_type::<u32>(options.endian())?;
		let compressed_size = reader.read_type::<u32>(options.endian())?;
		let uncompressed_size = reader.read_type::<u32>(options.endian())?;
		out.push((name, offset, compressed_size, uncompressed_size));

		remaining_header = remaining_header - (reader.stream_position()? - position) as u32;
	}
	Ok(out)
}

fn farc_entry_reader_compressed<R: io::Read + io::Seek>(
	reader: &mut R,
	_: &ReadOptions,
	entries: (Vec<(NullString, u32, u32, u32)>,),
) -> Result<Vec<Vec<u8>>, Error> {
	entries
		.0
		.iter()
		.map(|(_, offset, compressed_size, uncompressed_size)| {
			reader.seek(io::SeekFrom::Start(*offset as u64))?;
			let mut buf = vec![0u8; *compressed_size as usize];
			reader.read_exact(&mut buf)?;
			if compressed_size != uncompressed_size {
				let mut cursor = io::Cursor::new(buf);
				let mut decoder = libflate::gzip::Decoder::new(&mut cursor)?;
				let mut decoded = Vec::new();
				decoder.read_to_end(&mut decoded)?;
				buf = decoded;
			}
			Ok(buf)
		})
		.collect::<Result<_, _>>()
}

#[derive(Debug, BinRead)]
struct CompressedFarcHeaderReader {
	remaining_header: u32,
	alignment: i32,
	#[br(parse_with = farc_header_reader_compressed, args(remaining_header))]
	entries: Vec<(NullString, u32, u32, u32)>,
	#[br(parse_with = farc_entry_reader_compressed, args(entries.clone()))]
	data: Vec<Vec<u8>>,
}

fn farc_header_reader<R: io::Read + io::Seek>(
	reader: &mut R,
	options: &ReadOptions,
	header_length: (u32,),
) -> Result<Vec<(NullString, u32, u32)>, Error> {
	let mut remaining_header = header_length.0 - 4;
	let position = reader.stream_position()?;
	let mut out = vec![];
	while remaining_header > 0 {
		let name = reader.read_type::<NullString>(options.endian())?;
		let offset = reader.read_type::<u32>(options.endian())?;
		let size = reader.read_type::<u32>(options.endian())?;
		out.push((name, offset, size));

		remaining_header = remaining_header - (reader.stream_position()? - position) as u32;
	}
	Ok(out)
}

fn farc_entry_reader<R: io::Read + io::Seek>(
	reader: &mut R,
	_: &ReadOptions,
	entries: (Vec<(NullString, u32, u32)>,),
) -> Result<Vec<Vec<u8>>, Error> {
	entries
		.0
		.iter()
		.map(|(_, offset, size)| {
			reader.seek(io::SeekFrom::Start(*offset as u64))?;
			let mut buf = vec![0u8; *size as usize];
			reader.read_exact(&mut buf)?;
			Ok(buf)
		})
		.collect::<Result<_, _>>()
}

#[derive(Debug, BinRead)]
struct FarcHeaderReader {
	remaining_header: u32,
	alignment: i32,
	#[br(parse_with = farc_header_reader, args(remaining_header))]
	entries: Vec<(NullString, u32, u32)>,
	#[br(parse_with = farc_entry_reader, args(entries.clone()))]
	data: Vec<Vec<u8>>,
}

#[derive(Debug)]
pub enum FarcError {
	Io(io::Error),
	BinRead(binrw::Error),
	NulError(std::ffi::NulError),
	MissingData,
}

impl From<io::Error> for FarcError {
	fn from(value: io::Error) -> Self {
		Self::Io(value)
	}
}

impl From<binrw::Error> for FarcError {
	fn from(value: binrw::Error) -> Self {
		Self::BinRead(value)
	}
}

impl From<std::ffi::NulError> for FarcError {
	fn from(value: std::ffi::NulError) -> Self {
		Self::NulError(value)
	}
}

#[derive(Debug, Clone)]
pub struct Farc {
	pub entries: BTreeMap<String, Vec<u8>>,
}

impl Farc {
	pub fn read(path: &str) -> Result<Self, FarcError> {
		let bytes = std::fs::read(path)?;
		let mut reader = io::Cursor::new(bytes);
		let farc_file: FarcFileReader = reader.read_ne()?;
		let entries = match farc_file {
			FarcFileReader::Uncompressed(farc) => farc
				.entries
				.iter()
				.enumerate()
				.map(|(i, (name, _, _))| Some((name.to_string(), farc.data.get(i)?.clone())))
				.collect::<Option<BTreeMap<String, Vec<_>>>>()
				.ok_or(FarcError::MissingData)?,
			FarcFileReader::Compressed(farc) => farc
				.entries
				.iter()
				.enumerate()
				.map(|(i, (name, _, _, _))| Some((name.to_string(), farc.data.get(i)?.clone())))
				.collect::<Option<BTreeMap<String, Vec<_>>>>()
				.ok_or(FarcError::MissingData)?,
		};
		Ok(Self { entries })
	}

	pub fn write(self, path: &str, compress: bool) -> Result<(), FarcError> {
		let mut file = std::fs::File::create(path)?;
		let compressed_data = if compress {
			self.entries
				.iter()
				.map(|(_, data)| {
					let buf = vec![];
					let mut encoder = libflate::gzip::Encoder::new(buf)?;
					encoder.write(data)?;
					encoder.finish().into_result()
				})
				.collect::<Result<Vec<_>, _>>()?
		} else {
			vec![vec![]]
		};
		if compress {
			file.write(b"FArC")?;
		} else {
			file.write(b"FArc")?;
		}
		let size_position = file.stream_position()?;
		file.write_be(&0u32)?; // Remaining header
		file.write_be(&0u32)?; // Alignment
		let mut offsets = vec![];
		for (i, (name, data)) in self.entries.iter().enumerate() {
			file.write(std::ffi::CString::new(name.clone())?.as_bytes_with_nul())?;
			offsets.push(file.stream_position()?);
			file.write_be(&0u32)?; // Offset
			if compress {
				file.write_be(&(compressed_data[i].len() as u32))?;
			}
			file.write_be(&(data.len() as u32))?;
		}
		let header_length = file.stream_position()? - 8;
		file.seek(io::SeekFrom::Start(size_position))?;
		file.write_be(&(header_length as u32))?;
		file.seek(io::SeekFrom::Start(header_length + 8))?;
		for (i, (_, data)) in self.entries.iter().enumerate() {
			let position = file.stream_position()?;
			file.seek(io::SeekFrom::Start(offsets[i]))?;
			file.write_be(&(position as u32))?;
			file.seek(io::SeekFrom::Start(position))?;
			if compress {
				file.write(&compressed_data[i])?;
			} else {
				file.write(data)?;
			}
		}
		Ok(())
	}
}
