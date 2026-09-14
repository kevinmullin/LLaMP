fn main() {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("write-sources") => {
            let dir = xtask::fixture_dir();
            xtask::write_sources(&dir).unwrap_or_else(|err| {
                eprintln!("{err}");
                std::process::exit(1);
            });
            println!("{}", dir.display());
        }
        Some("emit") => {
            let dest = args
                .next()
                .unwrap_or_else(|| "assets/skins/dist/fixture.wsz".into());
            let bytes = xtask::fixture_wsz();
            if let Some(parent) = std::path::Path::new(&dest).parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent).expect("dist");
                }
            }
            std::fs::write(&dest, bytes).unwrap_or_else(|err| {
                eprintln!("{err}");
                std::process::exit(1);
            });
            println!("{dest}");
        }
        _ => {
            eprintln!("usage: xtask write-sources | xtask emit [out.wsz]");
            std::process::exit(2);
        }
    }
}
