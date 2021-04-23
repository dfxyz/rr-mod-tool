use std::env::args;
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

fn usage() {
    println!("Usage: ./rr-mod-tool -p format src dst");
    println!("   or: ./rr-mod-tool -u src dst");
    println!("Available formats: tex, bpe, pach, epac.")
}
