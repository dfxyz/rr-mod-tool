use std::convert::TryFrom;
use std::fs::{create_dir_all, File};
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

use crate::{
    create_file_to_write, list_files, read_exact, unpack_files, write_padding_zeroes,
    PackedFileInfo,
};

// TEX (align=16)
// [file_num: u32][reserved(?): u32*3]
// [file_name: u8*16]
// [ext: u8*4][len: u32][offset: u32][padding: u32]
// [data: ..]

const RESERVED: &[u8; 12] = b"\x00\x01\x00\x00\x00\x00\x00\x00\x10\x00\x00\x00";
const ALIGN_SIZE: u64 = 16;

pub fn pack(src_path: PathBuf, dst_path: PathBuf) {
    let file_info_list = list_files(&src_path, ALIGN_SIZE, None);
    let file_num = u32::try_from(file_info_list.len()).unwrap();
    assert!(file_num > 0);

    if let Some(dst_dir) = dst_path.parent() {
        create_dir_all(dst_dir).unwrap()
    }
    let file = create_file_to_write(dst_path);
    let mut writer = BufWriter::new(file);

    writer.write_all(&file_num.to_le_bytes()).unwrap();
    writer.write_all(RESERVED).unwrap();

    let mut global_offset = 16 + 32 * file_info_list.len();
    for info in &file_info_list {
        let filename = info.path.file_name().unwrap().to_string_lossy();
        let (filename, ext) = split_filename_and_ext(&filename);
        let filename = filename.as_bytes();
        let ext = ext.as_bytes();
        assert!(filename.len() > 0 && filename.len() <= 16);
        assert!(ext.len() > 0 && ext.len() <= 4);

        writer.write_all(filename).unwrap();
        write_padding_zeroes(&mut writer, 16 - filename.len());

        writer.write_all(ext).unwrap();
        write_padding_zeroes(&mut writer, 4 - ext.len());

        let len = u32::try_from(info.len).unwrap();
        writer.write_all(&len.to_le_bytes()).unwrap();

        let offset = u32::try_from(global_offset).unwrap();
        writer.write_all(&offset.to_le_bytes()).unwrap();

        write_padding_zeroes(&mut writer, 4);

        global_offset += (info.len + info.padding_zero_num) as usize;
    }

    for info in file_info_list {
        let mut file = File::open(info.path).unwrap();
        let mut vec = vec![];
        file.read_to_end(&mut vec).unwrap();
        writer.write_all(&vec).unwrap();
        write_padding_zeroes(&mut writer, info.padding_zero_num as _);
    }
}

#[inline]
fn split_filename_and_ext(filename: &str) -> (&str, &str) {
    match filename.rfind('.') {
        Some(i) => (&filename[..i], &filename[i + 1..]),
        None => (&filename, ""),
    }
}

pub fn unpack(src_path: PathBuf, dst_path: PathBuf) {
    let file = File::open(src_path).unwrap();
    let mut reader = BufReader::new(file);

    let buf = read_exact!(&mut reader, 4);
    let file_num = u32::from_le_bytes(buf);
    assert!(file_num > 0);

    reader.seek(SeekFrom::Current(12)).unwrap();

    let mut file_info_list = vec![];
    for _ in 0..file_num {
        let buf = read_exact!(&mut reader, 16);
        let filename = get_bytes_before_zero(&buf);

        let buf = read_exact!(&mut reader, 4);
        let ext = get_bytes_before_zero(&buf);

        let filename = format!(
            "{}.{}",
            String::from_utf8_lossy(filename),
            String::from_utf8_lossy(ext)
        );

        let buf = read_exact!(&mut reader, 4);
        let len = u32::from_le_bytes(buf);

        let buf = read_exact!(&mut reader, 4);
        let offset = u32::from_le_bytes(buf);

        reader.seek(SeekFrom::Current(4)).unwrap();

        file_info_list.push(PackedFileInfo {
            filename,
            offset: offset as _,
            len: len as _,
        });
    }

    create_dir_all(&dst_path).unwrap();
    unpack_files(&mut reader, &file_info_list, &dst_path);
}

#[inline]
fn get_bytes_before_zero(bytes: &[u8]) -> &[u8] {
    for i in 0..bytes.len() {
        if bytes[i] == 0 {
            return &bytes[..i];
        }
    }
    bytes
}
