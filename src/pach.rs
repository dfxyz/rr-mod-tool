use std::convert::TryFrom;
use std::fs::{create_dir_all, File};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use crate::{
    create_file_to_write, list_files, read_exact, unpack_files, write_padding_zeroes,
    PackedFileInfo,
};

// PACH (align=4)
// [magic_num: u32][file_num: u32]
// [file_no: u32][offset: u32][len: u32]
// ...
// [data: ..]

const MAGIC_NUM: &[u8; 4] = b"PACH";
const ALIGN_SIZE: u64 = 4;

pub fn detect_format<P: AsRef<Path>>(path: P) -> bool {
    let mut file = File::open(path).unwrap();
    let buf = read_exact!(&mut file, 4);
    &buf == MAGIC_NUM
}

pub fn pack(src_path: PathBuf, dst_path: PathBuf) {
    let file_info_list = list_files(
        &src_path,
        ALIGN_SIZE,
        Some(|filename| {
            let filename = filename.to_string_lossy();
            let filename = filename.as_bytes();
            for byte in filename {
                if *byte < b'0' || *byte > b'9' {
                    return false;
                }
            }
            true
        }),
    );
    let file_num = u32::try_from(file_info_list.len()).unwrap();
    assert!(file_num > 0);

    if let Some(dst_dir) = dst_path.parent() {
        create_dir_all(dst_dir).unwrap()
    }
    let file = create_file_to_write(dst_path);
    let mut writer = BufWriter::new(file);

    writer.write_all(MAGIC_NUM).unwrap();
    writer.write_all(&file_num.to_le_bytes()).unwrap();

    let mut global_offset = 0u32;
    for info in &file_info_list {
        let filename = info.path.file_name().unwrap().to_string_lossy();
        let file_no = u32::from_str(&filename).unwrap();
        writer.write_all(&file_no.to_le_bytes()).unwrap();

        let offset = global_offset;
        writer.write_all(&offset.to_le_bytes()).unwrap();

        let len = u32::try_from(info.len).unwrap();
        writer.write_all(&len.to_le_bytes()).unwrap();

        global_offset =
            u32::checked_add(global_offset, len + info.padding_zero_num as u32).unwrap();
    }
    for info in file_info_list {
        let mut file = File::open(info.path).unwrap();
        let mut vec = vec![];
        file.read_to_end(&mut vec).unwrap();
        writer.write_all(&vec).unwrap();
        write_padding_zeroes(&mut writer, info.padding_zero_num as _);
    }
}

pub fn unpack(src_path: PathBuf, dst_path: PathBuf) {
    let file = File::open(src_path).unwrap();
    let mut reader = BufReader::new(file);

    let buf = read_exact!(&mut reader, 4);
    assert_eq!(&buf, MAGIC_NUM);

    let buf = read_exact!(&mut reader, 4);
    let file_num = u32::from_le_bytes(buf);
    assert!(file_num > 0);
    let base_offset = 8 + file_num * 12;

    let mut file_info_list = vec![];
    for _ in 0..file_num {
        let mut buf = [0u8; 4];
        reader.read_exact(&mut buf).unwrap();
        let file_no = u32::from_le_bytes(buf);

        let mut buf = [0u8; 4];
        reader.read_exact(&mut buf).unwrap();
        let offset = u32::from_le_bytes(buf) + base_offset;

        let mut buf = [0u8; 4];
        reader.read_exact(&mut buf).unwrap();
        let len = u32::from_le_bytes(buf);

        file_info_list.push(PackedFileInfo {
            filename: file_no.to_string(),
            offset: offset as _,
            len: len as _,
        })
    }

    create_dir_all(&dst_path).unwrap();
    unpack_files(&mut reader, &file_info_list, &dst_path);
}
