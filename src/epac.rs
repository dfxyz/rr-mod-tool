use std::convert::TryFrom;
use std::fs::{create_dir_all, File};
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use crate::{
    create_file_to_write, read_exact, unpack_files, write_padding_zeroes, FileInfo, PackedFileInfo,
};

// EPAC (align=0x800)
// header: len=0x4000
// [magic_num: u32][?: u32][size: u32][reserved(?): u32]
// entry info (from 0x800): [E???][?: u32][offset_of_next_file: u32]
//                      or: [file_no: u32][offset(*2048): u32][len(*256): u32]
// data [..]
// footer: len=0x800

const MAGIC_NUM: &[u8; 4] = b"EPAC";
const ALIGN_SIZE: usize = 2048;
const RESERVED: &[u8; 4] = b"\x07\x00\x00\x00";

const FOOTER1: &[u8; 16] = b"EOP5/1.10\x00\x00\x00\x00\x00\x00\x00";

struct DividerInfo {
    name: [u8; 4],
    divider_unknown_field: [u8; 4],
}

enum EntryInfo {
    Divider(DividerInfo),
    PackedFile(PackedFileInfo),
    File(FileInfo),
}

pub fn detect_format<P: AsRef<Path>>(path: P) -> bool {
    let mut file = File::open(path).unwrap();
    let buf = read_exact!(&mut file, 4);
    &buf == MAGIC_NUM
}

pub fn pack(src_path: PathBuf, dst_path: PathBuf) {
    let mut header_unknown_field = [0u8; 4];
    let mut footer_unknown_field = [0u8];
    let mut entry_info_list = vec![];
    {
        let file = File::open(src_path.join("__entry__")).unwrap();
        let mut reader = BufReader::new(file);
        reader.read_exact(&mut header_unknown_field).unwrap();
        reader.read_exact(&mut footer_unknown_field).unwrap();

        let mut buf = vec![];
        reader.read_to_end(&mut buf).unwrap();
        let len = buf.len() / 4;
        assert_eq!(buf.len() % 4, 0);

        let mut i = 0;
        while i < len {
            let start = i * 4;
            let end = (i + 1) * 4;
            let mut name = [0u8; 4];
            name.clone_from_slice(&buf[start..end]);
            if name[0] == b'E' {
                assert!(i + 1 < len);
                let mut divider_unknown_field = [0u8; 4];
                divider_unknown_field.clone_from_slice(&buf[((i + 1) * 4)..((i + 2) * 4)]);
                entry_info_list.push(EntryInfo::Divider(DividerInfo {
                    name,
                    divider_unknown_field,
                }));
                i += 2;
            } else {
                let name = String::from_utf8_lossy(&name).to_string();
                let path = src_path.join(name);
                let len = File::open(&path).unwrap().metadata().unwrap().len();
                let padding_zero_num = {
                    let rem = len % ALIGN_SIZE as u64;
                    if rem == 0 {
                        0
                    } else {
                        ALIGN_SIZE as u64 - rem
                    }
                };
                entry_info_list.push(EntryInfo::File(FileInfo {
                    path,
                    len,
                    padding_zero_num,
                }));
                i += 1;
            }
        }
    }
    assert!(!entry_info_list.is_empty());

    let file = create_file_to_write(dst_path);
    let mut writer = BufWriter::new(file);
    writer.write_all(MAGIC_NUM).unwrap();
    writer.write_all(&header_unknown_field).unwrap();

    let mut size = 0;
    for info in &entry_info_list {
        if let EntryInfo::File(info) = info {
            size = u32::checked_add(
                size,
                u32::try_from(info.len + info.padding_zero_num).unwrap(),
            )
            .unwrap();
        }
    }
    writer.write_all(&size.to_le_bytes()).unwrap();
    writer.write_all(RESERVED).unwrap();

    // write 0 until 0x800;
    write_padding_zeroes(&mut writer, 0x800 - 16);

    // write entry info
    let mut offset_of_2k_block = 0u32;
    for info in &entry_info_list {
        match info {
            EntryInfo::Divider(info) => {
                writer.write_all(&info.name).unwrap();
                writer.write_all(&info.divider_unknown_field).unwrap();
                let offset = offset_of_2k_block;
                writer.write_all(&offset.to_le_bytes()).unwrap();
            }
            EntryInfo::File(info) => {
                let filename = info.path.file_name().unwrap().to_string_lossy();
                let filename = filename.as_bytes();
                writer.write_all(filename).unwrap();
                let offset = offset_of_2k_block;
                writer.write_all(&offset.to_le_bytes()).unwrap();

                assert_eq!((info.len + info.padding_zero_num) % 2048, 0);
                let rem = info.len % 256;
                let mut len = u32::try_from(info.len / 256).unwrap();
                if rem > 0 {
                    len += 1;
                }
                writer.write_all(&len.to_le_bytes()).unwrap();

                offset_of_2k_block +=
                    u32::try_from((info.len + info.padding_zero_num) / 2048).unwrap();
            }
            _ => unreachable!(),
        }
    }

    // write 0 until 0x4000;
    let pos = writer.seek(SeekFrom::Current(0)).unwrap();
    write_padding_zeroes(&mut writer, (0x4000 - pos) as _);

    // write data
    for info in entry_info_list {
        if let EntryInfo::File(info) = info {
            let mut file = File::open(info.path).unwrap();
            let mut vec = vec![];
            file.read_to_end(&mut vec).unwrap();
            writer.write_all(&vec).unwrap();
            write_padding_zeroes(&mut writer, info.padding_zero_num as _);
        }
    }

    // write footer
    writer.write_all(FOOTER1).unwrap();
    for _ in 16..0x400 {
        writer.write_all(b"\x00").unwrap();
    }
    writer.write_all(&footer_unknown_field).unwrap();
    for _ in 1..0x400 {
        writer.write_all(b"\x00").unwrap();
    }
}

pub fn unpack(src_path: PathBuf, dst_path: PathBuf) {
    let file = File::open(src_path).unwrap();
    let mut reader = BufReader::new(file);

    let buf = read_exact!(&mut reader, 4);
    assert_eq!(&buf, MAGIC_NUM);

    let buf = read_exact!(&mut reader, 4);
    let header_unknown_field = u32::from_le_bytes(buf);

    reader.seek(SeekFrom::End(-0x400)).unwrap();
    let buf = read_exact!(&mut reader, 1);
    let footer_unknown_field = u8::from_le_bytes(buf);

    reader.seek(SeekFrom::Start(0x800)).unwrap();
    let mut entry_info_list = vec![];
    loop {
        let buf = read_exact!(&mut reader, 4);
        if &buf[..4] == [0, 0, 0, 0] {
            break;
        }
        if buf[0] == b'E' {
            let name = buf;
            let divider_unknown_field = read_exact!(&mut reader, 4);
            reader.seek(SeekFrom::Current(4)).unwrap();
            entry_info_list.push(EntryInfo::Divider(DividerInfo {
                name,
                divider_unknown_field,
            }));
        } else {
            let file_no = buf;

            let buf = read_exact!(&mut reader, 4);
            let offset = u32::from_le_bytes(buf) * 2048 + 0x4000;

            let buf = read_exact!(&mut reader, 4);
            let len = u32::from_le_bytes(buf) * 256;

            entry_info_list.push(EntryInfo::PackedFile(PackedFileInfo {
                filename: String::from_utf8_lossy(&file_no).to_string(),
                offset: offset as _,
                len: len as _,
            }));
        }
    }

    assert!(!entry_info_list.is_empty());
    create_dir_all(&dst_path).unwrap();

    // write entry info
    {
        let path = dst_path.join("__entry__");
        let file = create_file_to_write(path);
        let mut writer = BufWriter::new(file);
        writer
            .write_all(&header_unknown_field.to_le_bytes())
            .unwrap();
        writer
            .write_all(&footer_unknown_field.to_le_bytes())
            .unwrap();
        for info in &entry_info_list {
            match info {
                EntryInfo::Divider(info) => {
                    writer.write_all(&info.name).unwrap();
                    writer.write_all(&info.divider_unknown_field).unwrap();
                }
                EntryInfo::PackedFile(info) => {
                    let name = info.filename.as_bytes();
                    assert_eq!(name.len(), 4);
                    writer.write_all(name).unwrap();
                }
                _ => unreachable!(),
            }
        }
    }

    // extract files
    for info in entry_info_list {
        if let EntryInfo::PackedFile(info) = info {
            let vec = vec![info];
            unpack_files(&mut reader, &vec, &dst_path);
        }
    }
}
