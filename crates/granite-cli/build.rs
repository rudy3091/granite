use std::path::Path;
use std::process::Command;

/// Builds the `web/` frontend (vite + pnpm) so its output can be embedded
/// into the binary via `rust-embed`. Requires `pnpm` on the dev machine's
/// PATH; end users of the built `granite` binary never need it.
fn main() {
    let web_dir = Path::new("../../web");

    println!("cargo:rerun-if-changed={}/src", web_dir.display());
    println!("cargo:rerun-if-changed={}/package.json", web_dir.display());
    println!(
        "cargo:rerun-if-changed={}/vite.config.js",
        web_dir.display()
    );

    run(web_dir, &["install"]);
    run(web_dir, &["run", "build"]);
}

fn run(dir: &Path, args: &[&str]) {
    let status = Command::new("pnpm").args(args).current_dir(dir).status();

    match status {
        Ok(s) if s.success() => {}
        Ok(s) => panic!("`pnpm {}` failed with {s}", args.join(" ")),
        Err(e) => panic!(
            "Failed to run `pnpm {}` in {}: {e}\n\
             Install pnpm (https://pnpm.io) to build the web editor frontend.",
            args.join(" "),
            dir.display()
        ),
    }
}
