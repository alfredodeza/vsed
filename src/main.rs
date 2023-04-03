fn main() {
    if let Err(e) = vsed::parse_args().and_then(vsed::run) {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}
