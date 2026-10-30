mod assets;
mod level;
mod music;
mod presentation;
mod sim;
mod ui;
mod verify;
use std::path::{Path, PathBuf};
fn main() -> anyhow::Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    match args.get(1).map(String::as_str) {
        Some("import") => {
            let root = args.get(2).map(Path::new).unwrap_or(Path::new("."));
            let out = args.get(3).map(Path::new).unwrap_or(Path::new("assets"));
            assets::import(root, out)?;
            assets::export_sprites(out)?;
            assets::export_gameplay_audio(out)?;
            let l = level::export(out)?;
            println!("{} niveaux reconstruits, sprites exportés.", l.len());
        }
        Some("verify") => {
            verify::run(args.get(2).map(Path::new).unwrap_or(Path::new("assets")))?;
        }
        Some("music") => {
            let root = args.get(2).map(Path::new).unwrap_or(Path::new("assets"));
            let secs = args.get(3).map(|v| v.parse()).transpose()?.unwrap_or(120);
            let only = args.get(4).map(|v| v.parse()).transpose()?;
            music::render(root, secs, only)?;
        }
        Some("rebuild") => {
            let root = args.get(2).map(Path::new).unwrap_or(Path::new("assets"));
            assets::export_sprites(root)?;
            assets::export_gameplay_audio(root)?;
            level::export(root)?;
        }
        Some("--help") => println!(
            "lemmings-amiga [play [assets] | import [dossier ADF] [nouvelle destination] | rebuild [assets] | --smoke image.png]"
        ),
        _ => {
            let smoke = if args.get(1).is_some_and(|a| a == "--smoke") {
                Some(PathBuf::from(
                    args.get(2)
                        .ok_or_else(|| anyhow::anyhow!("image de sortie requise"))?,
                ))
            } else {
                None
            };
            let bundled = std::env::current_exe()?
                .parent()
                .unwrap()
                .join("../Resources/assets");
            let root =
                if smoke.is_none() && args.get(1).is_some_and(|s| s == "play") && args.len() > 2 {
                    PathBuf::from(&args[2])
                } else if Path::new("assets/levels.json").exists() {
                    PathBuf::from("assets")
                } else if bundled.join("levels.json").exists() {
                    bundled
                } else {
                    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets")
                };
            macroquad::Window::from_config(
                macroquad::miniquad::conf::Conf {
                    window_title: "Lemmings · Amiga / Rust".into(),
                    window_width: 1280,
                    window_height: 720,
                    high_dpi: true,
                    platform: macroquad::miniquad::conf::Platform {
                        apple_gfx_api: macroquad::miniquad::conf::AppleGfxApi::Metal,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                ui::run(root, smoke),
            );
        }
    }
    Ok(())
}
