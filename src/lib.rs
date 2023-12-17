use std::ffi::OsStr;
use std::fs::{read_dir, File, OpenOptions};
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;

pub mod bpe;
pub mod epac;
pub mod pach;
pub mod tex;

struct FileInfo {
    path: PathBuf,
    len: u64,
    padding_zero_num: u64,
}

struct PackedFileInfo {
    filename: String,
    offset: u64,
    len: u64,
}

#[macro_export]
macro_rules! read_exact {
    ($reader:expr, $len:literal) => {{
        let mut buf = [0u8; $len];
        $reader.read_exact(&mut buf).unwrap();
        buf
    }};
}

fn list_files(
    dir_path: &PathBuf,
    align_size: u64,
    filter: Option<fn(&OsStr) -> bool>,
) -> Vec<FileInfo> {
    let mut vec = vec![];
    let dir = read_dir(dir_path).unwrap();
    for entry in dir {
        let entry = entry.unwrap();
        let metadata = entry.metadata().unwrap();
        if !metadata.file_type().is_file() {
            continue;
        }
        let filename = entry.file_name();
        let len = metadata.len();
        if len == 0 {
            continue;
        }
        if let Some(filter) = filter {
            if !filter(filename.as_os_str()) {
                continue;
            }
        }
        let padding_zero_num = if align_size > 0 {
            let rem = len % align_size;
            if rem > 0 {
                align_size - rem
            } else {
                0
            }
        } else {
            0
        };
        let path = dir_path.join(filename);
        vec.push(FileInfo {
            path,
            len,
            padding_zero_num,
        });
    }
    // Sort files numerically
    vec.sort_by(|i1, i2| {
        let name1 = i1.path.file_name().unwrap().to_string_lossy();
        let name2 = i2.path.file_name().unwrap().to_string_lossy();

        let num1 = u32::from_str(&name1).unwrap_or_default();
        let num2 = u32::from_str(&name2).unwrap_or_default();

        num1.cmp(&num2)
    });
    vec
}

#[inline]
fn write_padding_zeroes<W: Write>(writer: &mut W, zero_num: usize) {
    let zero = [0u8];
    for _ in 0..zero_num {
        writer.write_all(&zero).unwrap();
    }
}

#[inline]
fn create_file_to_write<P: AsRef<Path>>(path: P) -> File {
    OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)
        .unwrap()
}

fn unpack_files<R: Read + Seek>(
    reader: &mut R,
    info_list: &[PackedFileInfo],
    output_dir_path: &PathBuf,
) {
    for info in info_list {
        reader.seek(SeekFrom::Start(info.offset as u64)).unwrap();

        let dst_path = output_dir_path.join(&info.filename);
        let file = create_file_to_write(dst_path);
        let mut writer = BufWriter::new(file);

        let mut vec = Vec::with_capacity(info.len as _);
        unsafe { vec.set_len(info.len as _) };
        reader.read_exact(&mut vec).unwrap();
        writer.write_all(&vec).unwrap();
    }
}
