use std::env::args;
use std::path::PathBuf;

enum Format {
    TEX,
    BPE,
    PACH,
    EPAC,
}

fn main() {
    let args: Vec<String> = args().skip(1).collect();
    if args.len() != 3 && args.len() != 4 {
        usage();
        return;
    }

    let auto_detect_format = args.len() == 3;

    let pack_mode;
    let mut format= Format::TEX;
    let mut i = 0;
    if auto_detect_format {
        pack_mode = false;
    } else {
        pack_mode = match &args[i] {
            s if s == "-p" => true,
            s if s == "-u" => false,
            _ => {
                usage();
                return;
            }
        };
        i += 1;
    }

    if !auto_detect_format {
        format = match &args[i] {
            s if s == "tex" => Format::TEX,
            s if s == "bpe" => Format::BPE,
            s if s == "pach" => Format::PACH,
            s if s == "epac" => Format::EPAC,
            _ => {
                usage();
                return;
            }
        };
        i += 1;
    }

    let src_path = PathBuf::from(&args[i]);
    i += 1;

    if auto_detect_format {
        if rr_mod_tool::epac::detect_format(&src_path) {
            format = Format::EPAC
        } else if rr_mod_tool::pach::detect_format(&src_path) {
            format = Format::PACH
        } else if rr_mod_tool::bpe::detect_format(&src_path) {
            format = Format::BPE
        };
    }

    let dst_path = PathBuf::from(&args[i]);

    match format {
        Format::TEX => {
            if pack_mode {
                rr_mod_tool::tex::pack(src_path, dst_path);
            } else {
                rr_mod_tool::tex::unpack(src_path, dst_path);
            }
        }
        Format::BPE => {
            if pack_mode {
                rr_mod_tool::bpe::pack(src_path, dst_path);
            } else {
                rr_mod_tool::bpe::unpack(src_path, dst_path);
            }
        }
        Format::PACH => {
            if pack_mode {
                rr_mod_tool::pach::pack(src_path, dst_path);
            } else {
                rr_mod_tool::pach::unpack(src_path, dst_path);
            }
        }
        Format::EPAC => {
            if pack_mode {
                rr_mod_tool::epac::pack(src_path, dst_path);
            } else {
                rr_mod_tool::epac::unpack(src_path, dst_path);
            }
        }
    }
}

fn usage() {
    println!("Usage: ./rr-mod-tool -p format src dst");
    println!("   or: ./rr-mod-tool -u format src dst");
    println!("   or: ./rr-mod-tool -u src dst");
    println!();
    println!("Available formats: tex, bpe, pach, epac.")
}
