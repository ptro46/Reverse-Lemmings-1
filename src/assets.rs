//! Read-only import of the custom sequential filesystem used by these two disks.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::Path};

pub fn be16(b: &[u8], o: usize) -> usize {
    u16::from_be_bytes(b[o..o + 2].try_into().unwrap()) as usize
}
pub fn be32(b: &[u8], o: usize) -> usize {
    u32::from_be_bytes(b[o..o + 4].try_into().unwrap()) as usize
}
#[derive(Clone)]
pub struct Bitmap {
    pub w: usize,
    pub h: usize,
    pub rgba: Vec<u8>,
}
impl Bitmap {
    pub fn blank(w: usize, h: usize) -> Self {
        Self {
            w,
            h,
            rgba: vec![0; w * h * 4],
        }
    }
    pub fn save(&self, p: &Path) -> Result<()> {
        let mut enc = png::Encoder::new(fs::File::create(p)?, self.w as u32, self.h as u32);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header()?.write_image_data(&self.rgba)?;
        Ok(())
    }
}
#[derive(Serialize, Deserialize, Clone)]
pub struct ObjectInfo {
    pub id: usize,
    pub w: usize,
    pub h: usize,
    #[serde(default)]
    pub effect: usize,
    pub frames: usize,
    pub trigger_x: i32,
    pub trigger_y: i32,
    pub trigger_w: i32,
    pub trigger_h: i32,
}
#[derive(Serialize)]
struct Record {
    disk: String,
    name: String,
    offset: usize,
    packed_size: usize,
    size: usize,
    compressed: bool,
}

struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
    word: u32,
    left: u32,
    crc: u32,
}
impl Bits<'_> {
    fn read(&mut self, n: usize) -> Result<usize> {
        let mut v = 0;
        for _ in 0..n {
            if self.left == 0 {
                self.pos = self.pos.checked_sub(4).context("flux ByteKiller tronqué")?;
                self.word = be32(self.data, self.pos) as u32;
                self.crc ^= self.word;
                self.left = 32;
            }
            v = (v << 1) | (self.word & 1) as usize;
            self.word >>= 1;
            self.left -= 1;
        }
        Ok(v)
    }
}
/// Lemmings stores the reverse bitstream, XOR checksum and decoded size at EOF.
pub fn unpack(data: &[u8]) -> Result<Vec<u8>> {
    ensure!(data.len() >= 12, "ByteKiller trop court");
    let len = data.len();
    let size = be32(data, len - 4);
    ensure!(
        (1..=4_000_000).contains(&size),
        "taille ByteKiller invalide"
    );
    let word = be32(data, len - 12) as u32;
    ensure!(word != 0, "ancre ByteKiller absente");
    let left = 31 - word.leading_zeros();
    let mut bits = Bits {
        data,
        pos: len - 12,
        word: word & !(1u32 << left),
        left,
        crc: be32(data, len - 8) as u32 ^ word,
    };
    let mut out = vec![0; size];
    let mut p = size;
    while p > 0 {
        let (count, distance) = if bits.read(1)? == 0 {
            if bits.read(1)? == 0 {
                (bits.read(3)? + 1, 0)
            } else {
                (2, bits.read(8)?)
            }
        } else {
            match bits.read(2)? {
                0 => (3, bits.read(9)?),
                1 => (4, bits.read(10)?),
                2 => (bits.read(8)? + 1, bits.read(12)?),
                _ => (bits.read(8)? + 9, 0),
            }
        };
        ensure!(count <= p, "débordement ByteKiller");
        for _ in 0..count {
            p -= 1;
            out[p] = if distance == 0 {
                bits.read(8)? as u8
            } else {
                ensure!(p + distance < size, "référence ByteKiller invalide");
                out[p + distance]
            };
        }
    }
    ensure!(bits.crc == 0, "checksum ByteKiller incorrect");
    Ok(out)
}

/// Iterate all FORM records, including records concatenated in noises/introdata.
pub fn forms(data: &[u8]) -> Vec<(usize, &[u8])> {
    let mut out = Vec::new();
    let mut o = 0;
    while o + 12 <= data.len() {
        if &data[o..o + 4] == b"FORM" {
            let len = be32(data, o + 4);
            if len >= 4 && len <= data.len() - o - 8 {
                out.push((o, &data[o..o + 8 + len]));
            }
        }
        o += 2;
    }
    out
}
pub fn chunks(form: &[u8]) -> Result<BTreeMap<[u8; 4], &[u8]>> {
    ensure!(form.len() >= 12 && &form[..4] == b"FORM", "FORM attendu");
    let mut out = BTreeMap::new();
    let mut o = 12;
    while o + 8 <= form.len() {
        let n = be32(form, o + 4);
        ensure!(n <= form.len() - o - 8, "chunk IFF tronqué");
        out.insert(form[o..o + 4].try_into().unwrap(), &form[o + 8..o + 8 + n]);
        o += 8 + n + (n & 1);
    }
    Ok(out)
}
pub fn ilbm(form: &[u8]) -> Result<Bitmap> {
    let c = chunks(form)?;
    let h = c.get(b"BMHD").context("BMHD absent")?;
    ensure!(h.len() >= 20, "BMHD tronqué");
    let (w, height, planes, masking, compression) =
        (be16(h, 0), be16(h, 2), h[8] as usize, h[9], h[10]);
    ensure!(
        w > 0
            && height > 0
            && w * height <= 8_000_000
            && (1..=8).contains(&planes)
            && masking <= 2
            && compression <= 1,
        "ILBM non pris en charge"
    );
    let palette = c.get(b"CMAP").context("CMAP absente")?;
    let body = c.get(b"BODY").context("BODY absent")?;
    let stride = w.div_ceil(16) * 2;
    let rows = height * (planes + usize::from(masking == 1));
    let expected = stride * rows;
    let mut raw = Vec::with_capacity(expected);
    let mut p = 0;
    if compression == 0 {
        ensure!(body.len() >= expected, "BODY tronqué");
        raw.extend_from_slice(&body[..expected]);
    } else {
        while raw.len() < expected {
            ensure!(p < body.len(), "ByteRun1 tronqué");
            let n = body[p] as i8;
            p += 1;
            if n >= 0 {
                let n = n as usize + 1;
                ensure!(
                    p + n <= body.len() && raw.len() + n <= expected,
                    "littéral ByteRun1 invalide"
                );
                raw.extend_from_slice(&body[p..p + n]);
                p += n;
            } else if n != -128 {
                let count = (1 - n as i16) as usize;
                ensure!(
                    p < body.len() && raw.len() + count <= expected,
                    "répétition ByteRun1 invalide"
                );
                raw.resize(raw.len() + count, body[p]);
                p += 1;
            }
        }
    }
    let mut out = Bitmap::blank(w, height);
    for y in 0..height {
        for x in 0..w {
            let row = y * stride * (planes + usize::from(masking == 1));
            let bit = 0x80 >> (x & 7);
            let mut index = 0;
            for plane in 0..planes {
                if raw[row + plane * stride + x / 8] & bit != 0 {
                    index |= 1 << plane;
                }
            }
            ensure!(index * 3 + 2 < palette.len(), "palette ILBM trop courte");
            let dst = (y * w + x) * 4;
            out.rgba[dst..dst + 3].copy_from_slice(&palette[index * 3..index * 3 + 3]);
            out.rgba[dst + 3] = if (masking == 1 && raw[row + planes * stride + x / 8] & bit == 0)
                || (masking == 2 && index == be16(h, 12))
            {
                0
            } else {
                255
            };
        }
    }
    Ok(out)
}
pub fn svx(form: &[u8]) -> Result<(u32, Vec<u8>, usize, usize)> {
    let c = chunks(form)?;
    let h = c.get(b"VHDR").context("VHDR absent")?;
    ensure!(
        h.len() >= 20 && h[15] == 0,
        "8SVX compressé non pris en charge"
    );
    let rate = be16(h, 12) as u32;
    ensure!(rate > 0, "fréquence 8SVX nulle");
    let b = c.get(b"BODY").context("BODY absent")?;
    Ok((rate, b.to_vec(), be32(h, 0), be32(h, 4)))
}
pub fn wav(rate: u32, samples: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend(b"RIFF");
    out.extend((36 + samples.len() as u32).to_le_bytes());
    out.extend(b"WAVEfmt ");
    out.extend(16u32.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(rate.to_le_bytes());
    out.extend(rate.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(8u16.to_le_bytes());
    out.extend(b"data");
    out.extend((samples.len() as u32).to_le_bytes());
    out.extend(samples.iter().map(|s| s.wrapping_add(128)));
    out
}
/// Blitter graphics: complete planes, word-aligned rows, final plane is mask.
pub fn planar(
    data: &[u8],
    w: usize,
    h: usize,
    planes: usize,
    palette: &[[u8; 3]],
    masked: bool,
) -> Result<Bitmap> {
    let stride = w.div_ceil(16) * 2;
    let area = stride * h;
    ensure!(
        data.len() >= area * (planes + usize::from(masked)),
        "image planaire tronquée"
    );
    let mut out = Bitmap::blank(w, h);
    for y in 0..h {
        for x in 0..w {
            let o = y * stride + x / 8;
            let bit = 0x80 >> (x & 7);
            let mut i = 0;
            for p in 0..planes {
                if data[p * area + o] & bit != 0 {
                    i |= 1 << p;
                }
            }
            let dst = (y * w + x) * 4;
            out.rgba[dst..dst + 3].copy_from_slice(&palette[i]);
            out.rgba[dst + 3] = if masked {
                if data[planes * area + o] & bit != 0 {
                    255
                } else {
                    0
                }
            } else if i == 0 {
                0
            } else {
                255
            };
        }
    }
    Ok(out)
}

pub fn import(root: &Path, out: &Path) -> Result<()> {
    ensure!(
        !out.exists(),
        "La destination existe déjà : {} (choisir un nouveau dossier)",
        out.display()
    );
    let mut disks = fs::read_dir(root)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("adf")))
        .collect::<Vec<_>>();
    disks.sort();
    ensure!(
        disks.len() == 2,
        "Deux ADF attendus dans {}",
        root.display()
    );
    let mut files = BTreeMap::new();
    let mut records = Vec::new();
    // Validate everything before writing: no guessed file recovery or partial disk import.
    for path in disks {
        let disk = fs::read(&path)?;
        ensure!(
            disk.len() == 901120 && &disk[0x400..0x40c] == b"Reserved\0\0\0\0",
            "disque Lemmings DD non reconnu : {}",
            path.display()
        );
        let mut offset = 0x1600;
        let mut ended = false;
        for i in 1..192 {
            let entry = &disk[0x400 + i * 16..0x410 + i * 16];
            if entry.iter().all(|b| *b == 255) {
                ended = true;
                break;
            }
            let end = entry[..12]
                .iter()
                .position(|b| *b == 0)
                .context("nom sans terminaison")?;
            let name = std::str::from_utf8(&entry[..end])?.to_owned();
            ensure!(
                !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'),
                "nom invalide"
            );
            let size = be32(entry, 12);
            ensure!(
                size > 0 && size <= disk.len() - offset,
                "fichier hors disque : {name}"
            );
            let packed = &disk[offset..offset + size];
            let decoded = unpack(packed).ok();
            // Known compressed resources MUST pass checksum; raw IFF is never guessed.
            let expected = name.starts_with("Ground")
                || name.starts_with("Objects")
                || name.starts_with("Level")
                || name.starts_with("special")
                || name == "leveldata"
                || name.starts_with("panel")
                || name == "introdata";
            ensure!(
                !expected || decoded.is_some(),
                "échec décompression de {name}"
            );
            let compressed = decoded.is_some();
            let bytes = decoded.unwrap_or_else(|| packed.to_vec());
            records.push(Record {
                disk: path.file_name().unwrap().to_string_lossy().into_owned(),
                name: name.clone(),
                offset,
                packed_size: size,
                size: bytes.len(),
                compressed,
            });
            ensure!(
                files.insert(name, bytes).is_none(),
                "nom présent sur deux disques"
            );
            offset += size;
        }
        ensure!(ended, "répertoire sans terminaison");
    }
    fs::create_dir_all(out.join("raw"))?;
    fs::create_dir_all(out.join("images"))?;
    fs::create_dir_all(out.join("audio"))?;
    let mut images = 0;
    let mut sounds = 0;
    for (name, bytes) in &files {
        fs::write(out.join("raw").join(name), bytes)?;
        for (i, (_, form)) in forms(bytes).into_iter().enumerate() {
            let label = format!("{name}_{i:03}");
            match &form[8..12] {
                b"ILBM"
                    if chunks(form)?.contains_key(b"BMHD")
                        && chunks(form)?.contains_key(b"BODY")
                        && chunks(form)?.contains_key(b"CMAP") =>
                {
                    ilbm(form)
                        .with_context(|| label.clone())?
                        .save(&out.join("images").join(format!("{label}.png")))?;
                    images += 1;
                }
                b"8SVX" => {
                    let (rate, samples, _, _) = svx(form)?;
                    fs::write(
                        out.join("audio").join(format!("{label}.wav")),
                        wav(rate, &samples),
                    )?;
                    sounds += 1;
                }
                _ => {}
            }
        }
    }
    export_styles(&files, out)?;
    fs::write(
        out.join("manifest.json"),
        serde_json::to_vec_pretty(&records)?,
    )?;
    println!(
        "{} fichiers extraits, {} images IFF, {} sons WAV ; 5 styles de terrain décodés.",
        records.len(),
        images,
        sounds
    );
    Ok(())
}
fn export_styles(files: &BTreeMap<String, Vec<u8>>, out: &Path) -> Result<()> {
    let meta = files.get("leveldata").context("leveldata absent")?;
    ensure!(meta.len() >= 5 * 0x590, "leveldata tronqué");
    for style in 0..5 {
        let base = style * 0x590;
        let dir = out.join(format!("styles/{style}"));
        fs::create_dir_all(&dir)?;
        let colors = (0..16)
            .map(|i| {
                let v = be16(meta, base + i * 2);
                [
                    ((v >> 8) & 15) as u8 * 17,
                    ((v >> 4) & 15) as u8 * 17,
                    (v & 15) as u8 * 17,
                ]
            })
            .collect::<Vec<_>>();
        let ground = &files[&format!("Ground{}", style + 1)];
        let objects = &files[&format!("Objects{}", style + 1)];
        for i in 0..64 {
            let o = base + 0x290 + i * 12;
            let w = be16(meta, o);
            let h = be16(meta, o + 2);
            if w == 65535 || w == 0 {
                continue;
            }
            let p = be32(meta, o + 4)
                .checked_sub(0x75578)
                .context("pointeur terrain invalide")?;
            let b = planar(
                ground.get(p..).context("terrain hors limites")?,
                w,
                h,
                3,
                &colors[8..16],
                true,
            )?;
            b.save(&dir.join(format!("tile_{i:02}.png")))?;
        }
        let mut info = Vec::new();
        for i in 0..16 {
            let o = base + 0x70 + i * 34;
            let w = be16(meta, o + 6);
            let h = be16(meta, o + 8);
            if w == 0 || w == 65535 {
                continue;
            }
            let frames = be16(meta, o + 4).max(1);
            let frame_size = be16(meta, o + 10);
            let p = be32(meta, o + 26)
                .checked_sub(0x5caf0)
                .context("pointeur objet invalide")?;
            for f in 0..frames {
                let offset = p + f * frame_size;
                let b = planar(
                    objects.get(offset..).context("objet hors limites")?,
                    w,
                    h,
                    4,
                    &colors,
                    true,
                )?;
                b.save(&dir.join(format!("object_{i:02}_{f:02}.png")))?;
            }
            info.push(ObjectInfo {
                id: i,
                w,
                h,
                frames,
                effect: be16(meta, o + 24),
                trigger_x: be16(meta, o + 16) as i16 as i32 * 4,
                trigger_y: be16(meta, o + 18) as i16 as i32 * 4,
                trigger_w: be16(meta, o + 20) as i32 * 4,
                trigger_h: be16(meta, o + 22) as i32 * 4,
            });
        }
        fs::write(dir.join("objects.json"), serde_json::to_vec_pretty(&info)?)?;
    }
    Ok(())
}

pub fn export_sprites(root: &Path) -> Result<()> {
    let code = fs::read(root.join("raw/Code"))?;
    let meta = fs::read(root.join("raw/leveldata"))?;
    let palette = (0..16)
        .map(|i| {
            let v = be16(&meta, i * 2);
            [
                ((v >> 8) & 15) as u8 * 17,
                ((v >> 4) & 15) as u8 * 17,
                (v & 15) as u8 * 17,
            ]
        })
        .collect::<Vec<_>>();
    fs::create_dir_all(root.join("sprites"))?;
    for spec in &crate::presentation::ANIMATIONS {
        for dir in 0..if spec.left_offset > 0 { 2 } else { 1 } {
            for f in 0..spec.frames {
                let stride = spec.w.div_ceil(16) * 2 * spec.h * (spec.planes + 1);
                let off = spec.address - 0x400 + dir * spec.left_offset + f * stride;
                let image = planar(
                    code.get(off..).context("sprite hors limites")?,
                    spec.w,
                    spec.h,
                    spec.planes,
                    &palette,
                    true,
                )?;
                image.save(&root.join(format!("sprites/{}_{dir}_{f:02}.png", spec.name)))?;
            }
        }
    }
    Ok(())
}

/// Resolve the sample ID and note exactly as the 68000 sound-call wrapper does.
pub fn export_gameplay_audio(root: &Path) -> Result<()> {
    let code = fs::read(root.join("raw/Code"))?;
    let basic = fs::read(root.join("raw/BasicFX"))?;
    let full = fs::read(root.join("raw/FullFX"))?;
    let basic_forms = forms(&basic);
    let full_forms = forms(&full);
    let mut report = Vec::new();
    for spec in &crate::presentation::SOUNDS {
        let o = spec.call_address - 0x400;
        ensure!(
            code[o..o + 6]
                == [
                    0x3b,
                    0x7c,
                    (spec.command >> 8) as u8,
                    spec.command as u8,
                    0,
                    0x82
                ],
            "appel sonore original différent : {}",
            spec.name
        );
        let sample = (spec.command & 255) as usize;
        let note = (spec.command >> 8) as usize;
        let (bank, index, form) = if sample >= 20 {
            ("BasicFX", sample - 20, basic_forms[sample - 20].1)
        } else {
            ("FullFX", sample - 1, full_forms[sample - 1].1)
        };
        let (_, pcm, _, _) = svx(form)?;
        let period = be16(&code, 0x17c40 - 0x400 + (note - 1) * 2);
        ensure!(period > 0, "période Paula nulle");
        let rate = (3546895. / period as f64).round() as u32;
        fs::write(
            root.join(format!("audio/gameplay_{}.wav", spec.name)),
            wav(rate, &pcm),
        )?;
        report.push(serde_json::json!({"event":spec.name,"command":format!("0x{:04x}",spec.command),"call_address":format!("0x{:x}",spec.call_address),"bank":bank,"form_index":index,"sample_id":sample,"note":note,"period":period,"rate_hz":rate}));
    }
    fs::write(
        root.join("gameplay-audio.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    fs::write(
        root.join("animations.json"),
        serde_json::to_vec_pretty(&crate::presentation::ANIMATIONS.as_slice())?,
    )?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_malformed_pack() {
        for n in 0..64 {
            assert!(unpack(&vec![0; n]).is_err());
        }
    }
    #[test]
    fn unsigned_wav() {
        let w = wav(8000, &[128, 255, 0, 127]);
        assert_eq!(&w[44..], &[0, 127, 128, 255]);
        assert_eq!(
            u32::from_le_bytes(w[4..8].try_into().unwrap()) as usize,
            w.len() - 8
        );
    }
    #[test]
    fn plane_order_mask() {
        let p = [[0, 0, 0], [255, 0, 0]];
        let b = planar(&[0x80, 0, 0xc0, 0], 2, 1, 1, &p, true).unwrap();
        assert_eq!(&b.rgba[..], &[255, 0, 0, 255, 0, 0, 0, 255]);
    }
    #[test]
    fn truncated_iff() {
        let mut f = b"FORM\0\0\0\x0cILBMBODY\0\0\0\x20".to_vec();
        assert!(chunks(&f).is_err());
        f.clear();
        assert!(chunks(&f).is_err());
    }
}
