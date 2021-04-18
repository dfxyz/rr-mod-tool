use std::env::args;
use std::fs::{read_dir, remove_dir_all, remove_file};
use std::path::PathBuf;

fn main() {
    let mut args = args().skip(1);
    match args.next() {
        Some(s) if s == "-p" => {
            work_in_pack_mode(args);
        }
        Some(s) if s == "-u" => {
            work_in_unpack_mode(args);
        }
        Some(s) if s == "-pDLC" => {
            work_in_pack_dlc_dir_mode(args);
        }
        Some(s) if s == "-uDLC" => {
            work_in_unpack_dlc_dir_mode(args);
        }
        Some(s) if s == "-cDLC" => {
            work_in_clean_dlc_dir_mode(args);
        }
        Some(s) if s == "-pCH" => {
            work_in_pack_ch_pac_mode(args);
        }
        Some(s) if s == "-uCH" => {
            work_in_unpack_ch_pac_mode(args);
        }
        _ => {
            usage();
            return;
        }
    };
}

fn work_in_pack_mode<I: Iterator<Item = String>>(mut args: I) {
    let func = match args.next() {
        Some(s) if s == "tex" => rr_mod_tool::tex::pack,
        Some(s) if s == "bpe" => rr_mod_tool::bpe::pack,
        Some(s) if s == "pach" => rr_mod_tool::pach::pack,
        Some(s) if s == "epac" => rr_mod_tool::epac::pack,
        _ => {
            usage();
            return;
        }
    };
    let src_path = match args.next() {
        Some(s) => PathBuf::from(s),
        None => {
            usage();
            return;
        }
    };
    let dst_path = match args.next() {
        Some(s) => PathBuf::from(s),
        None => {
            usage();
            return;
        }
    };
    func(src_path, dst_path);
}

fn work_in_unpack_mode<I: Iterator<Item = String>>(mut args: I) {
    let src_path = match args.next() {
        Some(s) => PathBuf::from(s),
        None => {
            usage();
            return;
        }
    };
    let dst_path = match args.next() {
        Some(s) => PathBuf::from(s),
        None => {
            usage();
            return;
        }
    };
    if rr_mod_tool::epac::detect_format(&src_path) {
        rr_mod_tool::epac::unpack(src_path, dst_path);
    } else if rr_mod_tool::pach::detect_format(&src_path) {
        rr_mod_tool::pach::unpack(src_path, dst_path);
    } else if rr_mod_tool::bpe::detect_format(&src_path) {
        rr_mod_tool::bpe::unpack(src_path, dst_path);
    } else {
        rr_mod_tool::tex::unpack(src_path, dst_path);
    }
}

fn work_in_pack_dlc_dir_mode<I: Iterator<Item = String>>(mut args: I) {
    let mut path = match args.next() {
        Some(s) => PathBuf::from(s),
        None => {
            usage();
            return;
        }
    };
    let dir = read_dir(&path).unwrap();
    for entry in dir {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            path.push(entry.file_name());
            path.push("0");
            let names = ["0", "1", "2", "3"];
            for i in &names {
                let path = path.join(i);
                let path = path.join("tex");
                let file_path = path.join("waist.tex");
                let dir_path = path.join("waist.d");
                let _ = std::panic::catch_unwind(|| {
                    rr_mod_tool::tex::pack(dir_path, file_path);
                });
            }
        }
    }
}

fn work_in_unpack_dlc_dir_mode<I: Iterator<Item = String>>(mut args: I) {
    let mut path = match args.next() {
        Some(s) => PathBuf::from(s),
        None => {
            usage();
            return;
        }
    };
    let dir = read_dir(&path).unwrap();
    for entry in dir {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            path.push(entry.file_name());
            path.push("0");
            let names = ["0", "1", "2", "3"];
            for i in &names {
                let path = path.join(i);
                let path = path.join("tex");
                let file_path = path.join("waist.tex");
                let dir_path = path.join("waist.d");
                let _ = std::panic::catch_unwind(|| {
                    rr_mod_tool::tex::unpack(file_path, dir_path);
                });
            }
        }
    }
}

fn work_in_clean_dlc_dir_mode<I: Iterator<Item = String>>(mut args: I) {
    let mut path = match args.next() {
        Some(s) => PathBuf::from(s),
        None => {
            usage();
            return;
        }
    };
    let dir = read_dir(&path).unwrap();
    for entry in dir {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            path.push(entry.file_name());
            path.push("0");
            let names = ["0", "1", "2", "3"];
            for i in &names {
                let path = path.join(i);
                let path = path.join("tex");
                let path = path.join("waist.d");
                let _ = remove_dir_all(path);
            }
        }
    }
}

fn work_in_pack_ch_pac_mode<I: Iterator<Item = String>>(args: I) {
    let args = args.collect::<Vec<String>>();
    if args.len() < 2 {
        usage();
        return;
    }
    for file_no in &args[..args.len() - 1] {
        let unpacked_tex_dir_path = PathBuf::from(format!("{}.d/10.d", file_no));
        let tex_file_path = PathBuf::from(format!("{}.d/10_", file_no));
        rr_mod_tool::tex::pack(unpacked_tex_dir_path, tex_file_path.clone());

        let packed_tex_path = PathBuf::from(format!("{}.d/10", file_no));
        rr_mod_tool::bpe::pack(tex_file_path.clone(), packed_tex_path);
        remove_file(tex_file_path).unwrap();

        let pach_dir_path = PathBuf::from(format!("{}.d", file_no));
        let pach_file_path = PathBuf::from(file_no);
        rr_mod_tool::pach::pack(pach_dir_path, pach_file_path.clone());
    }
    let epac_file_path = PathBuf::from(&args[args.len() - 1]);
    rr_mod_tool::epac::pack(PathBuf::from("."), epac_file_path);
}

fn work_in_unpack_ch_pac_mode<I: Iterator<Item = String>>(mut args: I) {
    let src_path = match args.next() {
        Some(s) => PathBuf::from(s),
        None => {
            usage();
            return;
        }
    };
    let dst_path = match args.next() {
        Some(s) => PathBuf::from(s),
        None => {
            usage();
            return;
        }
    };
    rr_mod_tool::epac::unpack(src_path, dst_path.clone());
    let dir = read_dir(&dst_path).unwrap();
    for entry in dir {
        let entry = entry.unwrap();
        let filename = entry.file_name().to_string_lossy().to_string();
        if filename.starts_with('_') {
            continue;
        }

        let file_path = dst_path.join(&filename);
        let dir_path = dst_path.join(format!("{}.d", filename));
        rr_mod_tool::pach::unpack(file_path, dir_path.clone());

        let src_path = dir_path.join("10");
        let dst_path = dir_path.join("10_");
        if rr_mod_tool::bpe::detect_format(&src_path) {
            rr_mod_tool::bpe::unpack(src_path, dst_path.clone());

            let src_path = dst_path;
            let dst_path = dir_path.join("10.d");
            rr_mod_tool::tex::unpack(src_path.clone(), dst_path);

            remove_file(src_path).unwrap();
        }
    }
}

fn usage() {
    println!("Usage: ./rr-mod-tool -p format src dst");
    println!("   or: ./rr-mod-tool -u src dst");
    println!("   or: ./rr-mod-tool -pDLC dlc_dir");
    println!("   or: ./rr-mod-tool -uDLC dlc_dir");
    println!("   or: ./rr-mod-tool -cDLC dlc_dir");
    println!("   or: ./rr-mod-tool -uCH pac_file dst_dir");
    println!("   or: ./rr-mod-tool -pCH modified_file_no... pac_file");
    println!("Available formats: tex, bpe, pach, epac.")
}
