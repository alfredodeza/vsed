use std::error::Error;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use tempfile::NamedTempFile;

fn apply_change(line: &str) -> bool {
    println!("Apply? y/n -> {}", line);

    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();
    let result: bool = match input.trim().to_lowercase().as_str() {
        "y" => true,
        "n" => false,
        _ => {
            println!("Invalid input, please enter y or n");
            apply_change(line)
        }
    };
    // use the result variable to return a boolean
    result
}


// create a function that opens a file path and returns a BufReader
fn open(path: &str) -> Result<Box<dyn BufRead>, Box<dyn Error>> {
    // guard against any error from opening the file
    let file = match File::open(path) {
        Ok(file) => file,
        Err(e) => return Err(e.into()), // convert the error to a Box<dyn Error>
    };
    let buffer = Box::new(BufReader::new(file));
    Ok(buffer)
}

fn lazy_read_lines(path: &str) -> std::io::Result<impl Iterator<Item = std::io::Result<String>>> {
    let buffer = open(path).unwrap();

    let lines = buffer
        .lines()
        .map(|result| result.map(|line| line.trim().to_string()));
    Ok(lines)
}

fn clear_screen() {
    print!("{esc}[2J{esc}[1;1H", esc = 27 as char);
}


// create a function that reads a file and loops over each line. The function will need to keep track of preceeding and following lines when there is a match. The function will accept a file path and a VimSearch struct which contains the search pattern, replacement, and flags
fn read_file(path: String, search: &VimSearch) -> Result<(), Box<dyn Error>> {
    let tmp_file = NamedTempFile::new()?;
    let tmp_file_path = tmp_file.path().to_str().unwrap().to_owned();
    let mut line_number = 0;
    {
        let mut writer = BufWriter::new(&tmp_file);

        println!("Temp file path: {}", tmp_file_path);
        clear_screen();
        // loop over lines in the file and only print the lines that match the search pattern which may be using a regex
        for line in lazy_read_lines(&path)? {
            
            let line = line?;
            line_number += 1;
            if line.contains(&search.search_pattern) {
                let replaced = line.replace(&search.search_pattern, &search.replacement);
                if apply_change(&replaced) {
                    // append the line to the temporary file from tmp_file which is a NamedTempFile
                    // use the write method on the file handle
                    writeln!(writer, "{}", replaced)?;
                    clear_screen();
                    
                } else {
                    writeln!(writer, "{}", line)?;
                    clear_screen();
                }
            } else {
                println!("Line {line_number}: {line}");
                writeln!(writer, "{}", line)?;
            }
            writer.flush()?;
        }

    }
    tmp_file.persist(tmp_file_path.clone())?;    
    println!("Temp file path: {}", tmp_file_path);
    Ok(())
}

// Vim search String parsing
#[derive(Debug)]
pub struct VimSearch {
    pub search_pattern: String,
    pub replacement: String,
    pub flags: String,
    pub delimiter: char,
    pub string: String,
}

// add a method to the struct for parsing the search string
impl VimSearch {
    pub fn new(search: String) -> Result<VimSearch, Box<dyn Error>> {
        let delimiter = match Self::get_delimeter(&search) {
            Ok(value) => value,
            Err(e) => return Err(e),
        };

        let search_parts = search.split('/').collect::<Vec<&str>>();

        // assume at least 3 parts, Ok since we did validation in get_delimeter

        Ok(VimSearch {
            search_pattern: search_parts[1].to_string(),
            replacement: search_parts[2].to_string(),
            flags: search_parts[3].to_string(),
            delimiter,
            string: search,
        })
    }

    fn get_delimeter(search_string: &str) -> Result<char, Box<dyn Error>> {
        if !search_string.starts_with("s") {
            return Err("Search string must start with 's'".into());
        }
        if search_string.len() < 6 {
            return Err("Search string must have at least 6 characters".into());
        }
        // if it starts with s, it must have a delimiter as the next character so lets assume that and check
        // that there are at least 3 characters in the string
        // using unwrap here is fine because we guard against the length of the string being less than 6
        let assumed_delimiter = search_string.chars().nth(1).unwrap();
        // count how many times the assumed delimiter appears in the string
        let delimeter_count = search_string.matches(assumed_delimiter).count();
        if delimeter_count != 3 {
            return Err("Search string must have 3 delimiters, assumed '{c}' as the delimiter and only found {delimeter_count}".into());
        }

        Ok(assumed_delimiter)
    }
}

#[derive(Debug)]
pub struct Options {
    pub pattern: String,
    pub replacement: String,
    pub paths: Vec<String>,
    pub dry_run: bool,
    pub context: usize,
}

fn generate_paths(pattern: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let mut paths = vec![];
    for entry in glob::glob(pattern)? {
        if let Ok(path) = entry {
            paths.push(path.to_str().unwrap().to_owned());
        }
    }
    Ok(paths)
}

pub fn parse_args() -> Result<Options, Box<dyn Error>> {
    let matches = clap::App::new("vsed")
        .version("0.0.1")
        .author("Alfredo Deza")
        .about("Interactive sed for multiple files")
        .arg(
            clap::Arg::with_name("pattern")
                .value_name("PATTERN")
                .help("The sed expression to use. Example: \"s/foo/bar/g\"")
                .index(1)
                .required(true),
        )
        .arg(
            clap::Arg::with_name("files")
                .long("files")
                .value_name("FILES")
                .help("One or more files to perform replacements")
                .index(2)
                .multiple(true)
                .required(true),
        )
        .arg(
            clap::Arg::with_name("dry-run")
                .long("dry-run")
                .help("Do not actually perform the replacements"),
        )
        .get_matches();

    // if globbing is not enabled in the shell, the files argument will be a single string with * in it
    // so we need to expand it
    let files_from_arguments: Vec<&str> = matches.values_of("files").unwrap().collect();
    let files;
    if files_from_arguments.len() == 1 && files_from_arguments[0].contains('*') {
        files = match generate_paths(files_from_arguments[0]) {
            Ok(value) => value,
            Err(e) => return Err(e),
        };
    } else {
        // convert them to Vec<String>
        files = files_from_arguments.iter().map(|s| s.to_string()).collect();
    };

    Ok(Options {
        pattern: matches.value_of("pattern").unwrap().to_string(),
        //replacement: matches.value_of("replacement pattern").unwrap().to_string(),
        replacement: "".to_string(),
        //files: matches.values_of("files").unwrap().map(|s| s.to_string()).collect(),
        paths: files,
        dry_run: matches.is_present("dry-run"),
        context: 3,
    })
}

pub fn run(options: Options) -> Result<(), Box<dyn Error>> {
    // debug print the options
    println!("{:?}", options);
    let search = VimSearch::new(options.pattern)?;
    println!("{:?}", search);
    for path in options.paths {
        read_file(path, &search)?;
    }
    Ok(())
}
