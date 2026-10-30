use crate::{
    assets,
    level::{self, Level},
    sim::{State, World},
};
use anyhow::{Result, ensure};
use std::{fs, path::Path};
pub fn run(root: &Path) -> Result<serde_json::Value> {
    let levels: Vec<Level> = serde_json::from_slice(&fs::read(root.join("levels.json"))?)?;
    ensure!(levels.len() == 120, "120 niveaux attendus");
    let mut pixels = 0usize;
    let mut used = 0;
    for l in &levels {
        let image = level::load_png(&root.join(format!("levels/{:03}.png", l.number)))?;
        ensure!(image.w == 1600 && image.h == 160, "dimensions niveau");
        ensure!(
            l.objects.iter().any(|o| o.id == 1),
            "entrée absente : {}",
            l.name
        );
        ensure!(
            l.objects.iter().any(|o| o.id == 0),
            "sortie absente : {}",
            l.name
        );
        pixels += image
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[3] > 0)
            .count();
        let info = level::object_info(root, l.style)?;
        for obj in &l.objects {
            ensure!(info.iter().any(|m| m.id == obj.id), "objet inconnu");
        }
        used += l.objects.len();
    }
    let mut w = World::new(
        levels[0].clone(),
        level::load_png(&root.join("levels/000.png"))?,
        level::object_info(root, levels[0].style)?,
    );
    let mut assigned = false;
    for _ in 0..5100 {
        if !assigned && let Some(i) = w.lems.iter().position(|l| l.state == State::Walk) {
            assigned = w.assign(i, 7);
        }
        w.step();
        if w.finished() {
            break;
        }
    }
    ensure!(assigned, "tutoriel : aucun creuseur assigné");
    ensure!(
        w.saved >= w.level.required,
        "tutoriel : {} sauvés / {} requis (état {:?})",
        w.saved,
        w.level.required,
        w.lems
    );
    let music: Vec<serde_json::Value> =
        serde_json::from_slice(&fs::read(root.join("music/catalog.json"))?)?;
    ensure!(music.len() == 21, "21 musiques attendues");
    for i in 0..21 {
        let b = fs::read(root.join(format!("music/{i:02}.wav")))?;
        ensure!(
            b.starts_with(b"RIFF") && b.len() > 44144,
            "WAV musical invalide"
        );
        ensure!(b[44..].iter().any(|v| *v != 0), "musique silencieuse");
    }

    let code = fs::read(root.join("raw/Code"))?;
    let meta = fs::read(root.join("raw/leveldata"))?;
    let palette = (0..16)
        .map(|i| {
            let v = assets::be16(&meta, i * 2);
            [
                ((v >> 8) & 15) as u8 * 17,
                ((v >> 4) & 15) as u8 * 17,
                (v & 15) as u8 * 17,
            ]
        })
        .collect::<Vec<_>>();
    for (name, address) in crate::presentation::ANIMATION_CALLS {
        let spec = crate::presentation::ANIMATIONS
            .iter()
            .find(|s| s.name == name)
            .unwrap();
        let o = address - 0x400;
        ensure!(
            assets::be32(&code, o + 2) == spec.address,
            "pointeur original incorrect : {name}"
        );
        if name != "explosion" {
            ensure!(
                code[o..o + 2] == [0x21, 0x7c] && code[o + 6..o + 8] == [0, 0x26],
                "instruction originale incorrecte : {name}"
            );
        }
    }
    let mut checked_frames = 0;
    for spec in &crate::presentation::ANIMATIONS {
        for dir in 0..if spec.left_offset > 0 { 2 } else { 1 } {
            for f in 0..spec.frames {
                let p = spec.address - 0x400
                    + dir * spec.left_offset
                    + f * spec.w.div_ceil(16) * 2 * spec.h * (spec.planes + 1);
                let original =
                    assets::planar(&code[p..], spec.w, spec.h, spec.planes, &palette, true)?;
                let exported =
                    level::load_png(&root.join(format!("sprites/{}_{dir}_{f:02}.png", spec.name)))?;
                ensure!(
                    exported.w == spec.w && exported.h == spec.h && exported.rgba == original.rgba,
                    "animation différente du disque : {} / {dir} / {f}",
                    spec.name
                );
                checked_frames += 1;
            }
        }
    }
    for spec in &crate::presentation::SOUNDS {
        let bank = if spec.command & 255 >= 20 {
            "BasicFX"
        } else {
            "FullFX"
        };
        let index = if spec.command & 255 >= 20 {
            (spec.command & 255) - 20
        } else {
            (spec.command & 255) - 1
        } as usize;
        let raw = fs::read(root.join("raw").join(bank))?;
        let forms = assets::forms(&raw);
        let (_, pcm, _, _) = assets::svx(forms[index].1)?;
        let period = assets::be16(
            &code,
            0x17c40 - 0x400 + ((spec.command >> 8) as usize - 1) * 2,
        );
        let rate = (3546895. / period as f64).round() as u32;
        let exported = fs::read(root.join(format!("audio/gameplay_{}.wav", spec.name)))?;
        ensure!(
            exported == assets::wav(rate, &pcm),
            "son ou hauteur différent du disque : {}",
            spec.name
        );
    }
    let report = serde_json::json!({"animation_sequences_verified":crate::presentation::ANIMATIONS.len(),"animation_frames_verified":checked_frames,"gameplay_sounds_verified":crate::presentation::SOUNDS.len(),"levels":levels.len(),"placed_objects":used,"terrain_pixels":pixels,"tutorial_saved":w.saved,"tutorial_required":w.level.required,"tutorial_ticks":w.tick,"music_tracks":music.len(),"music_loops_detected":music.iter().filter(|v|v["loop_detected"]==true).count(),"raw_files":fs::read_dir(root.join("raw"))?.count(),"sounds_wav":fs::read_dir(root.join("audio"))?.count()});
    println!("{}", serde_json::to_string_pretty(&report)?);
    // Render a CPU-only view from the actual simulation; useful without WindowServer/GPU.
    let mut image = w.terrain.clone();
    for o in &w.level.objects {
        if let Ok(b) = level::load_png(&root.join(format!(
            "styles/{}/object_{:02}_00.png",
            w.level.style, o.id
        ))) {
            level::paste(&mut image, &b, o.x, o.y, false, false, false);
        }
    }
    image.save(&root.join("verified-tutorial.png"))?;
    // Exercise corrupt/truncated compressed inputs independently of real assets.
    ensure!(
        assets::unpack(&[0; 12]).is_err(),
        "décodeur accepte un flux invalide"
    );
    Ok(report)
}
