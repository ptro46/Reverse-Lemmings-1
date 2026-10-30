use crate::assets::{Bitmap, ObjectInfo, be16};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::Path};
pub const WIDTH: usize = 1600;
pub const HEIGHT: usize = 160;
#[derive(Clone, Serialize, Deserialize)]
pub struct Placement {
    pub id: usize,
    pub x: i32,
    pub y: i32,
    pub flags: usize,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Level {
    pub number: usize,
    pub name: String,
    pub rating: String,
    pub style: usize,
    pub special: usize,
    pub total: usize,
    pub required: usize,
    pub minutes: usize,
    pub release: usize,
    pub skills: [usize; 8],
    pub scroll: usize,
    pub objects: Vec<Placement>,
    pub steel: Vec<[i32; 4]>,
}
pub fn load_png(path: &Path) -> Result<Bitmap> {
    let decoder = png::Decoder::new(std::io::BufReader::new(fs::File::open(path)?));
    let mut r = decoder.read_info()?;
    let mut data = vec![0; r.output_buffer_size().context("PNG trop grand")?];
    let info = r.next_frame(&mut data)?;
    ensure!(info.color_type == png::ColorType::Rgba, "PNG RGBA attendu");
    data.truncate(info.buffer_size());
    Ok(Bitmap {
        w: info.width as usize,
        h: info.height as usize,
        rgba: data,
    })
}
pub fn paste(
    dst: &mut Bitmap,
    src: &Bitmap,
    x: i32,
    y: i32,
    flip: bool,
    behind: bool,
    erase: bool,
) {
    for sy in 0..src.h {
        for sx in 0..src.w {
            let dx = x + sx as i32;
            let dy = y + sy as i32;
            if dx < 0 || dy < 0 || dx >= dst.w as i32 || dy >= dst.h as i32 {
                continue;
            }
            let so = ((if flip { src.h - 1 - sy } else { sy }) * src.w + sx) * 4;
            let target = (dy as usize * dst.w + dx as usize) * 4;
            if src.rgba[so + 3] == 0 || (behind && dst.rgba[target + 3] > 0) {
                continue;
            }
            if erase {
                dst.rgba[target..target + 4].fill(0);
            } else {
                dst.rgba[target..target + 4].copy_from_slice(&src.rgba[so..so + 4]);
            }
        }
    }
}
pub fn export(root: &Path) -> Result<Vec<Level>> {
    let meta = fs::read(root.join("raw/leveldata"))?;
    let odd = fs::read(root.join("raw/oddtable"))?;
    let mut levels = Vec::new();
    let mut tiles = BTreeMap::new();
    for style in 0..5 {
        for i in 0..64 {
            let p = root.join(format!("styles/{style}/tile_{i:02}.png"));
            if p.exists() {
                tiles.insert((style, i), load_png(&p)?);
            }
        }
    }
    fs::create_dir_all(root.join("levels"))?;
    for number in 0..120 {
        // Table codes select one of 100 terrain layouts and its alternate metadata.
        let code = be16(&meta, 0x1bd0 + number * 2);
        let index = code >> 1;
        ensure!(index < 100, "index niveau invalide");
        let pack = fs::read(root.join(format!("raw/Level{:03}", index / 4)))?;
        let mut b = pack[index % 4 * 2048..(index % 4 + 1) * 2048].to_vec();
        if code & 1 == 0 {
            ensure!(
                index > 0 && index * 56 <= odd.len(),
                "index oddtable invalide"
            );
            let alt = &odd[(index - 1) * 56..index * 56];
            b[..24].copy_from_slice(&alt[..24]);
            b[2016..2048].copy_from_slice(&alt[24..56]);
        }
        let style = be16(&b, 26);
        ensure!(style < 5, "style invalide");
        let special = be16(&b, 28);
        let mut objects = Vec::new();
        for i in 0..32 {
            let o = 32 + i * 8;
            let r = &b[o..o + 8];
            if r.iter().all(|v| *v == 0) {
                continue;
            }
            objects.push(Placement {
                id: be16(&b, o + 4),
                x: be16(&b, o) as i16 as i32 - 16,
                y: be16(&b, o + 2) as i16 as i32,
                flags: be16(&b, o + 6),
            });
        }
        let mut terrain = Bitmap::blank(WIDTH, HEIGHT);
        if special > 0 {
            let data = fs::read(root.join(format!("raw/special{}", special - 1)))?;
            let form = crate::assets::forms(&data)
                .into_iter()
                .find(|(_, f)| &f[8..12] == b"ILBM")
                .context("image spéciale absente")?;
            let img = crate::assets::ilbm(form.1)?;
            paste(&mut terrain, &img, 304, 0, false, false, false);
        } else {
            for i in 0..400 {
                let o = 288 + i * 4;
                let r = &b[o..o + 4];
                if r.iter().all(|v| *v == 255) {
                    continue;
                }
                let x = (be16(&b, o) & 0xfff) as i32 - 16;
                let mut y = ((r[2] as i32) << 1) | (r[3] as i32 >> 7);
                if y >= 256 {
                    y -= 512;
                }
                y -= 4;
                let id = (r[3] & 63) as usize;
                let tile = tiles
                    .get(&(style, id))
                    .with_context(|| format!("tuile {style}/{id} absente"))?;
                paste(
                    &mut terrain,
                    tile,
                    x,
                    y,
                    r[3] & 64 != 0,
                    r[0] & 128 != 0,
                    r[0] & 64 != 0,
                );
            }
        }
        let steel = (0..32)
            .filter_map(|i| {
                let r = &b[1888 + i * 4..1892 + i * 4];
                if r.iter().all(|b| *b == 0) {
                    return None;
                }
                Some([
                    (((r[0] as i32) << 1) | ((r[1] as i32) >> 7)) * 4 - 16,
                    ((r[1] & 127) as i32) * 4 - 4,
                    ((r[2] >> 4) as i32 + 1) * 4,
                    ((r[2] & 15) as i32 + 1) * 4,
                ])
            })
            .collect();
        let mut skills = [0; 8];
        for (i, n) in skills.iter_mut().enumerate() {
            *n = be16(&b, 8 + i * 2);
        }
        let level = Level {
            number,
            name: String::from_utf8_lossy(&b[2016..2048]).trim().to_owned(),
            rating: ["Fun", "Tricky", "Taxing", "Mayhem"][number / 30].to_owned(),
            style,
            special,
            total: be16(&b, 2),
            required: be16(&b, 4),
            minutes: be16(&b, 6),
            release: be16(&b, 0),
            skills,
            scroll: be16(&b, 24),
            objects,
            steel,
        };
        terrain.save(&root.join(format!("levels/{number:03}.png")))?;
        levels.push(level);
    }
    fs::write(
        root.join("levels.json"),
        serde_json::to_vec_pretty(&levels)?,
    )?;
    Ok(levels)
}
pub fn object_info(root: &Path, style: usize) -> Result<Vec<ObjectInfo>> {
    Ok(serde_json::from_slice(&fs::read(
        root.join(format!("styles/{style}/objects.json")),
    )?)?)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paste_modes() {
        let mut d = Bitmap::blank(2, 2);
        let s = Bitmap {
            w: 1,
            h: 1,
            rgba: vec![1, 2, 3, 255],
        };
        paste(&mut d, &s, 0, 0, false, false, false);
        paste(&mut d, &s, -1, 0, false, false, false);
        let before = d.rgba.clone();
        paste(
            &mut d,
            &Bitmap {
                w: 1,
                h: 1,
                rgba: vec![9; 4],
            },
            0,
            0,
            false,
            true,
            false,
        );
        assert_eq!(before, d.rgba);
        paste(&mut d, &s, 0, 0, false, false, true);
        assert_eq!(d.rgba, vec![0; 16]);
    }
}
