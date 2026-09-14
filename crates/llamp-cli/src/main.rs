fn main() -> std::process::ExitCode {
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() == Some("render-skin") {
        let rest: Vec<String> = args.collect();
        return llamp_skin::render_skin_cli(&rest);
    }
    llamp_audio::cli::run()
}
