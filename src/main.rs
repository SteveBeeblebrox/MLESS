mod compilers;
use compilers::{compile_less, compile_html, CompileOptions};

use clap::{Arg, App};

use backtrace::Backtrace;

use std::io::prelude::*;
use std::path::PathBuf;
use std::path::Path;
use std::fs::File;
use std::fs;

use std::io::Read;
use std::io;
use std::panic;

fn main() {
    let matches = App::new("MLESS")
        .version(clap::crate_version!())
        .author(clap::crate_authors!())
        .about("A standalone Less compiler")

        .arg(Arg::with_name("output")
            .short("o")
            .long("out")
            .value_name("OUTPUT")
            .help("Sets the output file to write transpiled code to instead of using the input file's name with the extension changed to .css or .html in the case of HTML files (When set but blank, output is written to stdout; if set to a directory and an input file is provided, the output file will be written to the given directory with the extension changed to .css/.html)")
            .default_value("")
            .hide_default_value(true)
            .takes_value(true)
        )

        .arg(Arg::with_name("html")
            .long("html")
            .short("H")
            .help("Treat the input as an HTML file and transpile any script tags with the type attribute set to 'text/less'")
        )

        .arg(Arg::with_name("verbose")
            .short("v")
            .long("verbose")
            .help("Prints verbose error messages")
        )

        .arg(Arg::with_name("INPUT")
            .help("Sets the input file to compile (Leave blank to read from stdin)")
            .index(1)
        )
        .get_matches();

        let verbose = matches.occurrences_of("verbose") > 0;
        if cfg!(not(debug_assertions)) {
            panic::set_hook(Box::new(move |info| {
                println!("error: {}", panic_message::panic_info_message(info));
                
                if verbose {
                    println!("{:?}", Backtrace::new());
                } else {
                    println!("rerun with -v for verbose error messages");
                }
            }));
        }

        // Determine input file (or stdin)
        let (input_file, input_text) = match matches.value_of("INPUT") {
            Some(value) => (Some(String::from(value)), fs::read_to_string(value).expect("Error reading target file")),
            None => {
                let stdin = io::stdin();
                let mut stdin = stdin.lock();
                let mut line = String::new();

                stdin.read_to_string(&mut line).expect("Error reading stdin");
                (None, String::from(line))
            }
        };

        let html = matches.occurrences_of("html") > 0;

        let options = CompileOptions {
        };

        let result = if html {
            compile_html(input_text.as_str(), options).expect("Error compiling HTML")
        } else {
            compile_less(input_text.as_str(), options).expect("Error compiling Less")
        };

        match matches.value_of("output") {
            Some("") if matches.occurrences_of("output") > 0 => print!("{}", result.as_str()),
            None | Some("") => {
                match input_file {
                    Some(input_file) => {
                        let mut path = PathBuf::from(input_file);
                        path.set_extension(if html {"html"} else {"css"});
                        let mut file = File::create(path).expect("Error creating output file");
                        file.write_all(result.as_bytes()).expect("Error writing to output file");
                    },
                    None => print!("{}", result.as_str())
                }
            }
            Some(path) => {
                let path = if Path::new(path).exists() && fs::metadata(path).expect("Error reading file metadata").is_dir() && input_file.is_some() {
                    let mut path = PathBuf::from(path);
                    path.push(Path::new(&input_file.unwrap().to_string()).file_name().expect("Error getting file name").to_str().expect("Error getting file name"));
                    path.set_extension(if html {"html"} else {"css"});
                    path
                } else {
                    PathBuf::from(path)
                };

                let mut file = File::create(path).expect("Error creating output file");
                file.write_all(result.as_bytes()).expect("Error writing to output file");
            }
        }
}
