// BPE
// [magic_num: u32][reserved: u32][compressed_len: u32][decompressed_len: u32]
// block
// [encoding_info: ..][block_len: u16][block_data: ..]

extern crate threadpool;

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet, VecDeque};
use std::convert::TryFrom;
use std::fs::{create_dir_all, File};
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;

use crate::{create_file_to_write, read_exact};

const MAGIC_NUM: &[u8; 4] = b"BPE ";
const RESERVED: &[u8; 4] = b"\x00\x01\x00\x00";
const MAX_BLOCK_SIZE: usize = 4096;
const MAX_NORMAL_BYTE_NUM: usize = 200;
const MIN_OCCURRENCE: usize = 3;

pub fn detect_format<P: AsRef<Path>>(path: P) -> bool {
    let mut file = File::open(path).unwrap();
    let buf = read_exact!(&mut file, 4);
    &buf == MAGIC_NUM
}

pub fn pack(src_path: PathBuf, dst_path: PathBuf) {
    let file = File::open(src_path).unwrap();
    let file_len = u32::try_from(file.metadata().unwrap().len()).unwrap();
    let mut reader = BufReader::new(file);

    if let Some(p) = dst_path.parent() {
        create_dir_all(p).unwrap();
    }
    let file = create_file_to_write(dst_path);
    let mut writer = BufWriter::new(file);

    writer.write_all(MAGIC_NUM).unwrap();
    writer.write_all(RESERVED).unwrap();
    writer.write_all(b"\x00\x00\x00\x00").unwrap(); // re-write compressed_len later
    writer.write_all(&file_len.to_le_bytes()).unwrap();

    let pool = threadpool::ThreadPool::default();
    let mut rx_list = vec![];
    loop {
        match read_block_to_compress(&mut reader) {
            None => break,
            Some((block, used_bytes)) => {
                let (tx, rx) = channel::<Vec<u8>>();
                rx_list.push(rx);
                pool.execute(move || {
                    let compressed = compress_block(block, &used_bytes);
                    tx.send(compressed).unwrap();
                });
            }
        }
    }
    let mut compressed_len = 0;
    for rx in rx_list {
        let compressed = rx.recv().unwrap();
        writer.write_all(&compressed).unwrap();
        compressed_len += compressed.len();
    }
    let compressed_len = u32::try_from(compressed_len).unwrap();
    writer.seek(SeekFrom::Start(8)).unwrap();
    writer.write_all(&compressed_len.to_le_bytes()).unwrap();
}

fn read_block_to_compress<R: Read + Seek>(reader: &mut R) -> Option<(Vec<u8>, HashSet<u8>)> {
    let mut vec = vec![];
    let mut used_bytes = HashSet::new();
    loop {
        let mut buf = [0u8];
        let read_num = reader.read(&mut buf).unwrap();
        if read_num == 0 {
            return if vec.is_empty() {
                None
            } else {
                Some((vec, used_bytes))
            };
        }

        if used_bytes.len() == MAX_NORMAL_BYTE_NUM && !used_bytes.contains(&buf[0]) {
            reader.seek(SeekFrom::Current(-1)).unwrap();
            break;
        }

        vec.push(buf[0]);
        used_bytes.insert(buf[0]);
        if vec.len() >= MAX_BLOCK_SIZE {
            break;
        }
    }
    Some((vec, used_bytes))
}

fn compress_block(mut block: Vec<u8>, used_bytes: &HashSet<u8>) -> Vec<u8> {
    let mut substitutable_bytes = VecDeque::new();
    for b in 0..=u8::MAX {
        if !used_bytes.contains(&b) {
            substitutable_bytes.push_back(b);
        }
    }
    let mut count_map: HashMap<[u8; 2], usize> = HashMap::new();
    for slice in block.windows(2) {
        let pair = [slice[0], slice[1]];
        match count_map.get(&pair) {
            Some(count) => {
                let count = *count;
                count_map.insert(pair, count + 1);
            }
            None => {
                count_map.insert(pair, 1);
            }
        }
    }

    let mut substitution_map = HashMap::new();
    loop {
        let substituted_byte = match substitutable_bytes.pop_front() {
            Some(b) => b,
            None => break,
        };
        let pair = match count_map
            .iter()
            .filter(|(_, count)| **count >= MIN_OCCURRENCE)
            .max_by(|(pair1, count1), (pair2, count2)| {
                let result = count1.cmp(count2);
                if result != Ordering::Equal {
                    result
                } else {
                    let result = pair1[0].cmp(&pair2[0]);
                    if result != Ordering::Equal {
                        result
                    } else {
                        pair1[1].cmp(&pair2[1])
                    }
                }
            }) {
            Some((pair, _)) => [pair[0], pair[1]],
            None => break,
        };
        substitution_map.insert(substituted_byte, pair);

        let len = block.len();
        let mut w = 0; // index to write
        let mut r = 0; // index to read
        while r < len - 1 {
            if block[r] == pair[0] && block[r + 1] == pair[1] {
                if w > 0 {
                    let p = [block[w - 1], block[r]];
                    if let Some(count) = count_map.get(&p) {
                        let count = *count;
                        if count > 1 {
                            count_map.insert(p, count - 1);
                        } else {
                            count_map.remove(&p);
                        }
                    }
                    let p = [block[w - 1], substituted_byte];
                    match count_map.get(&p) {
                        Some(count) => {
                            let count = *count;
                            count_map.insert(p, count + 1);
                        }
                        None => {
                            count_map.insert(p, 1);
                        }
                    }
                }
                if r < len - 2 {
                    let p = [block[r + 1], block[r + 2]];
                    if let Some(count) = count_map.get(&p) {
                        let count = *count;
                        if count > 1 {
                            count_map.insert(p, count - 1);
                        } else {
                            count_map.remove(&p);
                        }
                    }
                    let p = [substituted_byte, block[r + 2]];
                    match count_map.get(&p) {
                        Some(count) => {
                            let count = *count;
                            count_map.insert(p, count + 1);
                        }
                        None => {
                            count_map.insert(p, 1);
                        }
                    }
                }
                block[w] = substituted_byte;
                w += 1;
                r += 2;
            } else {
                block[w] = block[r];
                w += 1;
                r += 1;
            }
        }
        if r == len - 1 {
            block[w] = block[r];
            w += 1;
        }
        block.truncate(w);
        count_map.remove(&pair);
    }

    let mut result = vec![];

    // write encoding info
    let mut byte = 0;
    'out: loop {
        let mut i = 1;
        if substitution_map.contains_key(&byte) {
            loop {
                let next = match u8::checked_add(byte, i) {
                    Some(b) => b,
                    None => {
                        // [byte, 255] are substituted
                        write_substituted_range(&mut result, byte, u8::MAX, &substitution_map);
                        break 'out;
                    }
                };
                if !substitution_map.contains_key(&next) {
                    break; // [byte+i] is not substituted
                }
                if i == 0x80 {
                    break; // reach limit, write substitution info of [byte, byte+0x7f]
                }
                i += 1;
            }
            // [byte, byte+i-1] are substituted
            write_substituted_range(&mut result, byte, byte + i - 1, &substitution_map);
            byte += i;
        } else {
            loop {
                let next = match u8::checked_add(byte, i) {
                    Some(b) => b,
                    None => {
                        // [byte, 255] are not substituted
                        write_not_substituted_range(&mut result, byte, u8::MAX, &substitution_map);
                        break 'out;
                    }
                };
                if substitution_map.contains_key(&next) {
                    break; // [byte+i] is substituted
                }
                if i == 0x80 {
                    break; // reach limit, write substitution info of [byte, byte+0x7f]
                }
                i += 1;
            }
            // [byte, byte+i-1] are not substituted
            write_not_substituted_range(&mut result, byte, byte + i - 1, &substitution_map);
            byte = match u8::checked_add(byte, i + 1) {
                Some(b) => b,
                None => break 'out, // done
            };
        }
    }

    // write compressed data's len
    let len = u16::try_from(block.len()).unwrap();
    result.extend_from_slice(&len.to_le_bytes());
    result.extend(block);
    result
}

#[inline]
fn write_substituted_range(
    buffer: &mut Vec<u8>,
    from: u8,
    to: u8,
    substitution_map: &HashMap<u8, [u8; 2]>,
) {
    let len = to - from;
    buffer.push(len);
    for b in from..=to {
        let pair = substitution_map.get(&b).unwrap();
        buffer.extend_from_slice(pair);
    }
}

#[inline]
fn write_not_substituted_range(
    buffer: &mut Vec<u8>,
    from: u8,
    to: u8,
    substitution_map: &HashMap<u8, [u8; 2]>,
) {
    let len = to - from + 0x80;
    buffer.push(len);
    if to < u8::MAX {
        match substitution_map.get(&(to + 1)) {
            Some(pair) => {
                buffer.extend_from_slice(pair);
            }
            None => {
                buffer.push(to + 1);
            }
        }
    }
}

pub fn unpack(src_path: PathBuf, dst_path: PathBuf) {
    let file = File::open(src_path).unwrap();
    let file_len = file.metadata().unwrap().len();
    let mut reader = BufReader::new(file);

    let buf = read_exact!(&mut reader, 4);
    assert_eq!(&buf, MAGIC_NUM);

    reader.seek(SeekFrom::Current(4)).unwrap();

    let buf = read_exact!(&mut reader, 4);
    let compressed_len = u32::from_le_bytes(buf);
    assert!(compressed_len > 0 && (compressed_len + 16) as u64 == file_len);

    let buf = read_exact!(&mut reader, 4);
    let decompressed_len = u32::from_le_bytes(buf);

    let file = create_file_to_write(dst_path);
    let mut writer = BufWriter::new(file);

    let mut total_read_num = 0;
    let mut total_write_num = 0;
    while total_read_num < compressed_len {
        let read_pos0 = reader.seek(SeekFrom::Current(0)).unwrap();
        let write_pos0 = writer.seek(SeekFrom::Current(0)).unwrap();

        unpack_one_block(&mut reader, &mut writer);

        let read_pos1 = reader.seek(SeekFrom::Current(0)).unwrap();
        let write_pos1 = writer.seek(SeekFrom::Current(0)).unwrap();

        total_read_num += u32::try_from(read_pos1 - read_pos0).unwrap();
        total_write_num += u32::try_from(write_pos1 - write_pos0).unwrap();
    }
    if total_write_num < decompressed_len {
        for _ in 0..(decompressed_len - total_write_num) {
            writer.write_all(b"\x00").unwrap();
        }
    }
}

fn unpack_one_block<R: Read, W: Write>(reader: &mut R, writer: &mut W) {
    let encoding_map = read_substitution_info(reader);

    let buf = read_exact!(reader, 2);
    let len = u16::from_le_bytes(buf);

    for _ in 0..len {
        let buf = read_exact!(reader, 1);
        match encoding_map.get(&buf[0]) {
            None => writer.write_all(&buf).unwrap(),
            Some(vec) => writer.write_all(&vec).unwrap(),
        }
    }
}

fn read_substitution_info<R: Read>(reader: &mut R) -> HashMap<u8, Vec<u8>> {
    let mut substituted_bytes = vec![];
    let mut substitution_map = HashMap::new();
    let mut byte = 0;
    'out: loop {
        let mut buf = [0u8; 2];
        reader.read_exact(&mut buf[..1]).unwrap();
        let mut substituted_byte_num = 1;
        if buf[0] >= 0x80 {
            // [byte, byte+i] are not substituted, (byte+i+1) may be substituted
            let i = buf[0] - 0x80;
            byte = match u8::checked_add(byte, i + 1) {
                Some(b) => b,
                None => break, // done
            };
        } else {
            // [byte, byte+i] may be substituted
            substituted_byte_num = buf[0] + 1;
        }
        for _ in 0..substituted_byte_num {
            reader.read_exact(&mut buf[..1]).unwrap();
            if byte != buf[0] {
                reader.read_exact(&mut buf[1..]).unwrap();
                substituted_bytes.push(byte);
                substitution_map.insert(byte, Vec::from(&buf[..]));
            }
            byte = match u8::checked_add(byte, 1) {
                Some(b) => b,
                None => break 'out, // done
            }
        }
    }

    for b in &substituted_bytes {
        flat_substitution_map(&mut substitution_map, *b);
    }
    substitution_map
}

fn flat_substitution_map(substitution_map: &mut HashMap<u8, Vec<u8>>, byte: u8) -> Vec<u8> {
    let vec;
    match substitution_map.get(&byte) {
        Some(v) => {
            vec = v.clone();
        }
        None => return vec![byte],
    }
    let mut result = vec![];
    for b in vec {
        result.extend(flat_substitution_map(substitution_map, b));
    }
    substitution_map.insert(byte, result.clone());
    result
}
