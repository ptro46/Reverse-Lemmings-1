use crate::{
    assets::Bitmap,
    level::{self, Level},
    sim::{State, World},
};
use macroquad::{
    audio::{self, PlaySoundParams, Sound},
    prelude::*,
};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
const INK: Color = Color::new(0.035, 0.047, 0.075, 1.);
const PANEL: Color = Color::new(0.065, 0.082, 0.115, 1.);
const MINT: Color = Color::new(0.48, 0.94, 0.68, 1.);
const DIM: Color = Color::new(0.52, 0.58, 0.65, 1.);
const SKILLS: [&str; 8] = [
    "CLIMBER", "FLOATER", "BOMBER", "BLOCKER", "BUILDER", "BASHER", "MINER", "DIGGER",
];
fn texture(b: &Bitmap) -> Texture2D {
    let t = Texture2D::from_rgba8(b.w as u16, b.h as u16, &b.rgba);
    t.set_filter(FilterMode::Nearest);
    t
}
fn file_texture(p: &Path) -> anyhow::Result<Texture2D> {
    Ok(texture(&level::load_png(p)?))
}
fn label(s: &str, x: f32, y: f32, size: f32, c: Color) {
    draw_text(s, x, y, size, c);
}
fn hit(r: Rect, m: Vec2) -> bool {
    r.contains(m)
}
fn button(r: Rect, text: &str, m: Vec2, active: bool) -> bool {
    let hover = hit(r, m);
    draw_rectangle(
        r.x,
        r.y,
        r.w,
        r.h,
        if active {
            MINT
        } else if hover {
            Color::new(0.14, 0.19, 0.23, 1.)
        } else {
            PANEL
        },
    );
    label(
        text,
        r.x + 14.,
        r.y + r.h / 2. + 6.,
        20.,
        if active { INK } else { WHITE },
    );
    hover && is_mouse_button_pressed(MouseButton::Left)
}
struct Game {
    w: World,
    terrain: Texture2D,
    objects: BTreeMap<(usize, usize), Texture2D>,
    camera: f32,
    paused: bool,
    fast: bool,
    selected: usize,
}
impl Game {
    fn load(root: &Path, l: &Level) -> anyhow::Result<Self> {
        let bitmap = level::load_png(&root.join(format!("levels/{:03}.png", l.number)))?;
        let info = level::object_info(root, l.style)?;
        let terrain = texture(&bitmap);
        let mut objects = BTreeMap::new();
        for m in &info {
            for f in 0..m.frames {
                objects.insert(
                    (m.id, f),
                    file_texture(
                        &root.join(format!("styles/{}/object_{:02}_{f:02}.png", l.style, m.id)),
                    )?,
                );
            }
        }
        let selected = l.skills.iter().position(|n| *n > 0).unwrap_or(0);
        Ok(Self {
            w: World::new(l.clone(), bitmap, info),
            terrain,
            objects,
            camera: l.scroll.min(960) as f32,
            paused: false,
            fast: false,
            selected,
        })
    }
}
pub async fn run(root: PathBuf, smoke: Option<PathBuf>) {
    if let Err(e) = app(root, smoke).await {
        eprintln!("{e:#}");
        loop {
            clear_background(INK);
            label("Impossible de charger le jeu", 50., 80., 32., WHITE);
            label(&format!("{e:#}"), 50., 130., 20., MINT);
            label(
                "Lancer d'abord : cargo run -- import",
                50.,
                180.,
                24.,
                WHITE,
            );
            if is_key_pressed(KeyCode::Escape) {
                break;
            }
            next_frame().await;
        }
    }
}
async fn app(root: PathBuf, smoke: Option<PathBuf>) -> anyhow::Result<()> {
    let levels: Vec<Level> = serde_json::from_slice(&fs::read(root.join("levels.json"))?)?;
    let menu = file_texture(&root.join("images/Icons_000.png"))?;
    let mut sprites = BTreeMap::new();
    for e in fs::read_dir(root.join("sprites"))? {
        let p = e?.path();
        if p.extension().is_some_and(|e| e == "png") {
            sprites.insert(
                p.file_stem().unwrap().to_string_lossy().into_owned(),
                file_texture(&p)?,
            );
        }
    }
    let mut sounds = Vec::<Sound>::new();
    for spec in &crate::presentation::SOUNDS {
        sounds.push(
            audio::load_sound(
                &root
                    .join("audio")
                    .join(format!("gameplay_{}.wav", spec.name))
                    .to_string_lossy(),
            )
            .await?,
        );
    }

    let mut music: Option<Sound> = None;
    let mut muted = false;
    let data_root = if cfg!(target_os = "macos") {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|p| p.join("Library/Application Support/LemmingsRust"))
    } else if cfg!(target_os = "windows") {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .map(|p| p.join("LemmingsRust"))
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .map(|p| p.join(".local/share"))
            })
            .map(|p| p.join("lemmings-rust"))
    }
    .unwrap_or_else(|| root.parent().unwrap_or(&root).to_path_buf());
    let save_path = data_root.join("progress.json");
    let mut completed: Vec<usize> = fs::read(&save_path)
        .ok()
        .and_then(|s| serde_json::from_slice(&s).ok())
        .unwrap_or_default();
    let mut rating = 0usize;
    let mut page = 0usize;
    let mut previews = BTreeMap::new();
    let mut game: Option<Game> = None;
    let mut accumulator = 0f32;
    let mut frames = 0;
    if smoke.is_some() {
        game = Some(Game::load(&root, &levels[0])?);
    }
    loop {
        let sw = screen_width();
        let sh = screen_height();
        let scale = (sw / 1280.).min(sh / 720.);
        let offset = vec2((sw - 1280. * scale) / 2., (sh - 720. * scale) / 2.);
        let (mx, my) = mouse_position();
        let mouse = (vec2(mx, my) - offset) / scale;
        set_camera(&Camera2D {
            zoom: vec2(2. * scale / sw, 2. * scale / sh),
            target: vec2(640., 360.),
            ..Default::default()
        });
        clear_background(INK);
        let mut restart = false;
        let mut leave = false;
        let mut next = false;
        if let Some(g) = game.as_mut() {
            let level = g.w.level.clone();
            label(
                &format!("{}  /  {:02}", level.rating, level.number % 30 + 1),
                32.,
                40.,
                18.,
                MINT,
            );
            label(&level.name, 32., 78., 34., WHITE);
            let remaining = (level.minutes as u32 * 60).saturating_sub(g.w.tick / 17);
            label(
                &format!(
                    "HOME {:02} / {:02}    OUT {:02}    {:02}:{:02}",
                    g.w.saved,
                    level.required,
                    g.w.lems
                        .iter()
                        .filter(|l| !matches!(l.state, State::Dead | State::Saved))
                        .count(),
                    remaining / 60,
                    remaining % 60
                ),
                720.,
                57.,
                25.,
                WHITE,
            );
            label(
                "CLICK A LEMMING TO ASSIGN  /  1-8 SELECT  /  ARROWS SCROLL",
                32.,
                110.,
                16.,
                DIM,
            );
            if is_key_pressed(KeyCode::Space) {
                g.paused = !g.paused;
            }
            if is_key_pressed(KeyCode::F) {
                g.fast = !g.fast;
            }
            if is_key_pressed(KeyCode::R) {
                restart = true;
            }
            if is_key_pressed(KeyCode::Escape) {
                leave = true;
            }
            for (i, k) in [
                KeyCode::Key1,
                KeyCode::Key2,
                KeyCode::Key3,
                KeyCode::Key4,
                KeyCode::Key5,
                KeyCode::Key6,
                KeyCode::Key7,
                KeyCode::Key8,
            ]
            .iter()
            .enumerate()
            {
                if is_key_pressed(*k) {
                    g.selected = i;
                }
            }
            let scroll = (if is_key_down(KeyCode::Right) { 1. } else { 0. })
                - (if is_key_down(KeyCode::Left) { 1. } else { 0. });
            g.camera = (g.camera + scroll * get_frame_time() * 400.).clamp(0., 960.);
            if mouse.y >= 128. && mouse.y <= 448. {
                if mouse.x < 15. {
                    g.camera = (g.camera - get_frame_time() * 350.).max(0.);
                }
                if mouse.x > 1265. {
                    g.camera = (g.camera + get_frame_time() * 350.).min(960.);
                }
            }
            if !g.paused && !g.w.finished() {
                accumulator += get_frame_time().min(0.1) * if g.fast { 4. } else { 1. };
                while accumulator >= 1. / 17. {
                    g.w.step();
                    accumulator -= 1. / 17.;
                }
            } else {
                accumulator = 0.;
            }
            if g.w.dirty {
                g.terrain.update(&Image {
                    bytes: g.w.terrain.rgba.clone(),
                    width: 1600,
                    height: 160,
                });
                g.w.dirty = false;
            }
            draw_rectangle(0., 128., 1280., 320., BLACK);
            draw_texture_ex(
                &g.terrain,
                0.,
                128.,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(1280., 320.)),
                    source: Some(Rect::new(g.camera, 0., 640., 160.)),
                    ..Default::default()
                },
            );
            for o in &level.objects {
                let Some(info) = g.w.info.iter().find(|m| m.id == o.id) else {
                    continue;
                };
                let frame = if o.id == 1 {
                    ((g.w.tick / 4) as usize).min(info.frames - 1)
                } else {
                    (g.w.tick as usize / 2) % info.frames
                };
                if let Some(t) = g.objects.get(&(o.id, frame)) {
                    let x = (o.x as f32 - g.camera) * 2.;
                    if (-160. ..=1280.).contains(&x) {
                        draw_texture_ex(
                            t,
                            x,
                            128. + o.y as f32 * 2.,
                            WHITE,
                            DrawTextureParams {
                                dest_size: Some(vec2(info.w as f32 * 2., info.h as f32 * 2.)),
                                flip_y: o.flags & 0x8000 == 0,
                                ..Default::default()
                            },
                        );
                    }
                }
            }
            let world_mouse = vec2(mouse.x / 2. + g.camera, (mouse.y - 128.) / 2.);
            let hovered =
                g.w.lems
                    .iter()
                    .enumerate()
                    .filter(|(_, l)| !matches!(l.state, State::Dead | State::Saved))
                    .min_by_key(|(_, l)| {
                        let dx = l.x - world_mouse.x as i32;
                        let dy = l.y - 5 - world_mouse.y as i32;
                        dx * dx + dy * dy
                    })
                    .and_then(|(i, l)| {
                        if (l.x as f32 - world_mouse.x).abs() < 9.
                            && (l.y as f32 - 5. - world_mouse.y).abs() < 12.
                        {
                            Some(i)
                        } else {
                            None
                        }
                    });
            for (i, l) in g.w.lems.iter().enumerate() {
                if matches!(l.state, State::Dead | State::Saved) {
                    continue;
                }
                let Some((animation, dir, f)) = crate::presentation::frame(l) else {
                    continue;
                };
                let key = format!("{}_{dir}_{f:02}", animation.name);
                if let Some(t) = sprites.get(&key) {
                    let x = (l.x as f32 - t.width() / 2. - g.camera) * 2.;
                    let y = 128. + (l.y as f32 - t.height() + 1.) * 2.;
                    if (-32.0..1280.).contains(&x) {
                        draw_texture_ex(
                            t,
                            x,
                            y,
                            WHITE,
                            DrawTextureParams {
                                dest_size: Some(vec2(t.width() * 2., t.height() * 2.)),
                                ..Default::default()
                            },
                        );
                        if hovered == Some(i) {
                            draw_circle_lines(
                                (l.x as f32 - g.camera) * 2.,
                                128. + (l.y as f32 - 5.) * 2.,
                                15.,
                                1.,
                                MINT,
                            );
                        }
                        if let Some(b) = l.bomb {
                            label(
                                &format!("{}", b.div_ceil(17).max(1)),
                                x + 8.,
                                y - 4.,
                                16.,
                                WHITE,
                            );
                        }
                    }
                }
            }
            if mouse.y >= 128.
                && mouse.y < 448.
                && is_mouse_button_pressed(MouseButton::Left)
                && let Some(i) = hovered
            {
                g.w.assign(i, g.selected);
            }
            for (i, skill_name) in SKILLS.iter().enumerate() {
                let r = Rect::new(24. + i as f32 * 154., 474., 144., 90.);
                if button(r, skill_name, mouse, g.selected == i) {
                    g.selected = i;
                }
                label(
                    &format!("{}", g.w.skills[i]),
                    r.x + 16.,
                    r.y + 76.,
                    32.,
                    if g.selected == i { INK } else { WHITE },
                );
                label(
                    &format!("{}", i + 1),
                    r.x + 116.,
                    r.y + 76.,
                    18.,
                    if g.selected == i { INK } else { DIM },
                );
            }
            if button(
                Rect::new(24., 588., 120., 44.),
                if g.paused { "RESUME" } else { "PAUSE" },
                mouse,
                g.paused,
            ) {
                g.paused = !g.paused;
            }
            if button(Rect::new(156., 588., 120., 44.), "FAST x4", mouse, g.fast) {
                g.fast = !g.fast;
            }
            if button(Rect::new(288., 588., 120., 44.), "RETRY", mouse, false) {
                restart = true;
            }
            if button(Rect::new(420., 588., 120., 44.), "NUKE", mouse, g.w.nuke) {
                g.w.nuke = true;
                g.w.released = level.total;
            }
            if button(Rect::new(552., 588., 52., 44.), "-", mouse, false) {
                g.w.release = g.w.release.saturating_sub(5).max(level.release);
            }
            label(&format!("RATE {}", g.w.release), 620., 617., 20., DIM);
            if button(Rect::new(738., 588., 52., 44.), "+", mouse, false) {
                g.w.release = (g.w.release + 5).min(99);
            }
            if button(
                Rect::new(818., 588., 130., 44.),
                if muted { "SOUND OFF" } else { "SOUND ON" },
                mouse,
                false,
            ) {
                muted = !muted;
                if let Some(s) = &music {
                    audio::set_sound_volume(s, if muted { 0. } else { 0.35 });
                }
            }
            if button(Rect::new(1100., 588., 150., 44.), "LEVELS", mouse, false) {
                leave = true;
            }
            draw_texture_ex(
                &g.terrain,
                24.,
                656.,
                Color::new(0.6, 0.7, 0.7, 1.),
                DrawTextureParams {
                    dest_size: Some(vec2(1232., 40.)),
                    ..Default::default()
                },
            );
            draw_rectangle_lines(
                24. + g.camera / 1600. * 1232.,
                656.,
                640. / 1600. * 1232.,
                40.,
                1.,
                MINT,
            );
            if hit(Rect::new(24., 656., 1232., 40.), mouse)
                && is_mouse_button_down(MouseButton::Left)
            {
                g.camera = ((mouse.x - 24.) / 1232. * 1600. - 320.).clamp(0., 960.);
            }
            for event in g.w.events.drain(..) {
                if !muted && let Some(s) = sounds.get(event as usize) {
                    audio::play_sound_once(s);
                }
            }
            if g.w.finished() {
                let won = g.w.saved >= level.required;
                if won && !completed.contains(&level.number) {
                    completed.push(level.number);
                    if let Err(e) = fs::create_dir_all(&data_root).and_then(|_| {
                        fs::write(
                            &save_path,
                            serde_json::to_vec_pretty(&completed).expect("serializable progress"),
                        )
                    }) {
                        eprintln!("Sauvegarde: {e}");
                    }
                }
                draw_rectangle(300., 188., 680., 210., Color::new(0.03, 0.05, 0.08, 0.98));
                label(
                    if won {
                        "EVERY LITTLE LEMMING COUNTS."
                    } else {
                        "LET'S TRY THAT AGAIN."
                    },
                    332.,
                    239.,
                    28.,
                    MINT,
                );
                label(
                    &format!("{} saved / {} required", g.w.saved, level.required),
                    332.,
                    280.,
                    24.,
                    WHITE,
                );
                if button(Rect::new(332., 314., 180., 50.), "TRY AGAIN", mouse, false) {
                    restart = true;
                }
                if button(Rect::new(536., 314., 180., 50.), "LEVELS", mouse, false) {
                    leave = true;
                }
                if won && button(Rect::new(740., 314., 206., 50.), "NEXT LEVEL", mouse, true) {
                    next = true;
                }
            }
        } else {
            label("AMIGA  /  1991", 40., 42., 16., MINT);
            label("A little chaos.", 40., 95., 46., WHITE);
            label("A lot of Lemmings.", 40., 142., 46., WHITE);
            label(
                "Original disk graphics. A new Rust engine.",
                42.,
                178.,
                20.,
                DIM,
            );
            draw_texture_ex(
                &menu,
                850.,
                18.,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(384., 125.)),
                    source: Some(Rect::new(0., 0., 640., 208.)),
                    ..Default::default()
                },
            );
            for i in 0..4 {
                if button(
                    Rect::new(40. + i as f32 * 200., 208., 184., 44.),
                    ["FUN", "TRICKY", "TAXING", "MAYHEM"][i],
                    mouse,
                    rating == i,
                ) {
                    rating = i;
                    page = 0;
                    previews.clear();
                }
            }
            for row in 0..3 {
                for col in 0..4 {
                    let slot = page * 12 + row * 4 + col;
                    if slot >= 30 {
                        continue;
                    }
                    let index = rating * 30 + slot;
                    let l = &levels[index];
                    let r = Rect::new(
                        40. + col as f32 * 310.,
                        278. + row as f32 * 116.,
                        294.,
                        102.,
                    );
                    let hover = hit(r, mouse);
                    draw_rectangle(
                        r.x,
                        r.y,
                        r.w,
                        r.h,
                        if hover {
                            Color::new(0.12, 0.17, 0.2, 1.)
                        } else {
                            PANEL
                        },
                    );
                    if let std::collections::btree_map::Entry::Vacant(e) = previews.entry(index) {
                        e.insert(file_texture(&root.join(format!("levels/{index:03}.png")))?);
                    }
                    draw_texture_ex(
                        &previews[&index],
                        r.x + 10.,
                        r.y + 8.,
                        WHITE,
                        DrawTextureParams {
                            dest_size: Some(vec2(274., 34.)),
                            ..Default::default()
                        },
                    );
                    label(
                        &format!(
                            "{:02}  {}",
                            slot + 1,
                            if completed.contains(&index) {
                                "SAVED"
                            } else {
                                ""
                            }
                        ),
                        r.x + 12.,
                        r.y + 64.,
                        17.,
                        MINT,
                    );
                    let text = if l.name.chars().count() > 30 {
                        format!("{}...", l.name.chars().take(27).collect::<String>())
                    } else {
                        l.name.clone()
                    };
                    label(&text, r.x + 12., r.y + 89., 16., WHITE);
                    if hover && is_mouse_button_pressed(MouseButton::Left) {
                        game = Some(Game::load(&root, l)?);
                        accumulator = 0.;
                        if let Some(s) = music.take() {
                            audio::stop_sound(&s);
                        }
                        let path = root.join(format!("music/{:02}.wav", index % 21));
                        if path.exists() {
                            let s = audio::load_sound(&path.to_string_lossy()).await?;
                            audio::play_sound(
                                &s,
                                PlaySoundParams {
                                    looped: true,
                                    volume: if muted { 0. } else { 0.35 },
                                },
                            );
                            music = Some(s);
                        }
                    }
                }
            }
            if button(Rect::new(40., 650., 110., 42.), "PREV", mouse, false) {
                page = page.saturating_sub(1);
                previews.clear();
            }
            label(&format!("{} / 3", page + 1), 180., 677., 18., DIM);
            if button(Rect::new(270., 650., 110., 42.), "NEXT", mouse, false) {
                page = (page + 1).min(2);
                previews.clear();
            }
            label(
                &format!("{} / 120 LEVELS SAVED", completed.len()),
                950.,
                678.,
                18.,
                MINT,
            );
            if is_key_pressed(KeyCode::Escape) {
                break;
            }
        }
        if restart && let Some(g) = &game {
            game = Some(Game::load(&root, &g.w.level)?);
            accumulator = 0.;
        }
        if next && let Some(g) = &game {
            let n = (g.w.level.number + 1).min(119);
            game = Some(Game::load(&root, &levels[n])?);
            accumulator = 0.;
        }
        if leave {
            game = None;
            previews.clear();
            if let Some(s) = music.take() {
                audio::stop_sound(&s);
            }
        }
        set_default_camera();
        if is_key_pressed(KeyCode::F12)
            && let Some(g) = &game
        {
            snapshot(&root, g)?.save(&std::env::temp_dir().join("lemmings-rust.png"))?;
        }
        frames += 1;
        if let Some(path) = &smoke
            && frames == 60
        {
            if let Some(g) = &game {
                snapshot(&root, g)?.save(path)?;
            }
            break;
        }
        next_frame().await;
    }
    Ok(())
}

// CPU capture avoids the Metal backend's unsupported texture readback.
fn snapshot(root: &Path, g: &Game) -> anyhow::Result<Bitmap> {
    let mut image = g.w.terrain.clone();
    for o in &g.w.level.objects {
        let Some(info) = g.w.info.iter().find(|m| m.id == o.id) else {
            continue;
        };
        let f = if o.id == 1 {
            ((g.w.tick / 4) as usize).min(info.frames - 1)
        } else {
            g.w.tick as usize / 2 % info.frames
        };
        let b = level::load_png(&root.join(format!(
            "styles/{}/object_{:02}_{f:02}.png",
            g.w.level.style, o.id
        )))?;
        level::paste(
            &mut image,
            &b,
            o.x,
            o.y,
            o.flags & 0x8000 == 0,
            false,
            false,
        );
    }
    for l in &g.w.lems {
        if matches!(l.state, State::Dead | State::Saved) {
            continue;
        }
        let Some((animation, dir, f)) = crate::presentation::frame(l) else {
            continue;
        };
        let b =
            level::load_png(&root.join(format!("sprites/{}_{dir}_{f:02}.png", animation.name)))?;
        level::paste(
            &mut image,
            &b,
            l.x - b.w as i32 / 2,
            l.y - b.h as i32 + 1,
            false,
            false,
            false,
        );
    }
    let mut view = Bitmap::blank(640, 160);
    level::paste(&mut view, &image, -g.camera as i32, 0, false, false, false);
    Ok(view)
}
