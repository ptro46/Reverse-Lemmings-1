//! The original music driver runs in a bounded 68000 interpreter; only Paula audio is emulated.
use crate::assets::{be16, be32};
use anyhow::{Context, Result, ensure};
use m68000::{M68000, cpu_details::Mc68000, memory_access::MemoryAccess};
use std::{collections::HashMap, fs, num::Wrapping, path::Path};
#[derive(Clone, Default)]
struct Channel {
    address: usize,
    end: usize,
    phase: f64,
    enabled: bool,
}
struct Paula {
    ram: Vec<u8>,
    dma: u16,
    channels: [Channel; 4],
}
impl Paula {
    fn new() -> Self {
        Self {
            ram: vec![0; 0x1000000],
            dma: 0,
            channels: Default::default(),
        }
    }
    fn word(&self, o: usize) -> u16 {
        be16(&self.ram, o) as u16
    }
    fn sample(&mut self) -> i16 {
        let mut mix = 0f64;
        for i in 0..4 {
            let reg = 0xdff0a0 + i * 16;
            let period = self.word(reg + 6).max(124) as f64;
            let vol = self.word(reg + 8).min(64) as f64;
            if !self.channels[i].enabled {
                continue;
            }
            let c = &mut self.channels[i];
            let a = c.address;
            if a < self.ram.len() {
                mix += self.ram[a] as i8 as f64 * vol;
            }
            c.phase += 3546895. / period / 22050.;
            while c.phase >= 1. {
                c.phase -= 1.;
                c.address += 1;
                if c.address >= c.end {
                    let ptr = be32(&self.ram, reg) & 0x1ffffe;
                    let n = be16(&self.ram, reg + 4) * 2;
                    c.address = ptr;
                    c.end = ptr + n.max(2);
                }
            }
        }
        (mix * 0.75).clamp(-32768., 32767.) as i16
    }
}
impl MemoryAccess for Paula {
    fn get_byte(&mut self, a: u32) -> Option<u8> {
        self.ram.get((a & 0xffffff) as usize).copied()
    }
    fn get_word(&mut self, a: u32) -> Option<u16> {
        let a = (a & 0xffffff) as usize;
        let b = self.ram.get(a..a + 2)?;
        Some(u16::from_be_bytes(b.try_into().ok()?))
    }
    fn set_byte(&mut self, a: u32, v: u8) -> Option<()> {
        *self.ram.get_mut((a & 0xffffff) as usize)? = v;
        Some(())
    }
    fn set_word(&mut self, a: u32, v: u16) -> Option<()> {
        let a = (a & 0xffffff) as usize;
        if a == 0xdff096 {
            let old = self.dma;
            let flags = v & 0x7fff;
            if v & 0x8000 != 0 {
                self.dma |= flags;
            } else {
                self.dma &= !flags;
            }
            for i in 0..4 {
                let on = self.dma & (1 << i) != 0;
                if on && old & (1 << i) == 0 {
                    let reg = 0xdff0a0 + i * 16;
                    let ptr = be32(&self.ram, reg) & 0x1ffffe;
                    let n = be16(&self.ram, reg + 4) * 2;
                    self.channels[i] = Channel {
                        address: ptr,
                        end: ptr + n.max(2),
                        phase: 0.,
                        enabled: true,
                    };
                } else if !on {
                    self.channels[i].enabled = false;
                }
            }
        }
        self.ram
            .get_mut(a..a + 2)?
            .copy_from_slice(&v.to_be_bytes());
        Some(())
    }
    fn reset_instruction(&mut self) {}
}
fn call(cpu: &mut M68000<Mc68000>, mem: &mut Paula, address: u32) -> Result<()> {
    cpu.regs.ssp = Wrapping(0x1efffc);
    cpu.regs.usp = Wrapping(0x1efffc);
    mem.ram[0x1efffc..0x1f0000].copy_from_slice(&0x00fffff0u32.to_be_bytes());
    cpu.regs.pc = Wrapping(address);
    for _ in 0..200_000 {
        if cpu.regs.pc.0 == 0xfffff0 {
            return Ok(());
        }
        let (_, ex) = cpu.interpreter_exception(mem);
        ensure!(
            ex.is_none(),
            "exception 68000 {:?} à {:x}",
            ex,
            cpu.regs.pc.0
        );
    }
    anyhow::bail!(
        "lecteur musical : limite d'instructions à {:x}",
        cpu.regs.pc.0
    )
}
pub fn render(root: &Path, seconds: usize, only: Option<usize>) -> Result<()> {
    ensure!(
        (1..=600).contains(&seconds),
        "durée entre 1 et 600 secondes"
    );
    let code = fs::read(root.join("raw/Code"))?;
    let meta = fs::read(root.join("raw/leveldata"))?;
    fs::create_dir_all(root.join("music"))?;
    let mut catalog = Vec::new();
    for track in 0..21 {
        if only.is_some_and(|i| i != track) {
            continue;
        }
        let entry = &meta[0x20ec + track * 28..0x2108 + track * 28];
        let name = String::from_utf8_lossy(&entry[..12])
            .trim_end_matches('\0')
            .to_owned();
        let tune = fs::read(root.join("raw").join(&name))?;
        let mut mem = Paula::new();
        mem.ram[0x400..0x400 + code.len()].copy_from_slice(&code);
        mem.ram[0x30000..0x30000 + tune.len()].copy_from_slice(&tune);
        let mut ptr = 0x40000;
        let mut previous = 0;
        let mut list = 0x28000;
        for id in &entry[12..28] {
            if *id != 0 {
                let s = 0x1d38 + (*id as usize - 1) * 12;
                let name = String::from_utf8_lossy(&meta[s..s + 12])
                    .trim_end_matches('\0')
                    .trim_end_matches(char::from(1))
                    .to_owned();
                // Last byte of each 12-byte instrument entry records compression, not the name.
                let end = name
                    .bytes()
                    .position(|c| !c.is_ascii_alphanumeric())
                    .unwrap_or(name.len());
                let data = fs::read(root.join("raw").join(&name[..end]))
                    .with_context(|| format!("instrument {id}: {name:?}"))?;
                ensure!(ptr + data.len() < 0x180000, "samples trop grands");
                mem.ram[ptr..ptr + data.len()].copy_from_slice(&data);
                previous = ptr;
                ptr += (data.len() + 1) & !1;
            }
            mem.ram[list..list + 4].copy_from_slice(&(previous as u32).to_be_bytes());
            list += 4;
        }
        mem.ram[list..list + 4].fill(0);
        let mut cpu = M68000::<Mc68000>::new_no_reset();
        call(&mut cpu, &mut mem, 0x17244)?;
        cpu.regs.a[0] = Wrapping(0x28000);
        cpu.regs.d[0] = Wrapping(1);
        call(&mut cpu, &mut mem, 0x1724c)?;
        cpu.regs.a[2] = Wrapping(0x30000);
        cpu.regs.d[0] = Wrapping(1);
        call(&mut cpu, &mut mem, 0x17250)?;
        cpu.regs.d[0] = Wrapping(1);
        call(&mut cpu, &mut mem, 0x1725c)?;
        let mut pcm = Vec::with_capacity(seconds * 22050 * 2);
        let mut deadline = 0f64;
        let mut ticks = 0;
        let mut seen = HashMap::new();
        let mut loop_start = None;
        while pcm.len() / 2 < seconds * 22050 {
            let state = mem.ram[0x17b1e..0x18726].to_vec();
            if let Some((sample, tick)) = seen.get(&state) {
                if ticks - *tick > 200 {
                    loop_start = Some(*sample);
                    break;
                }
            } else {
                seen.insert(state, (pcm.len(), ticks));
            }
            call(&mut cpu, &mut mem, 0x17284)?;
            ticks += 1;
            let timer = (mem.ram[0xbfe401] as usize) | ((mem.ram[0xbfe501] as usize) << 8);
            ensure!(timer > 0, "timer CIA nul");
            deadline += timer as f64 / 709379. * 22050.;
            while ((pcm.len() / 2) as f64) < deadline && pcm.len() / 2 < seconds * 22050 {
                pcm.extend(mem.sample().to_le_bytes());
            }
        }
        if let Some(start) = loop_start {
            pcm.drain(..start);
        }
        ensure!(
            pcm.iter().any(|b| *b != 0),
            "rendu musical silencieux : {name}"
        );
        let pcm_duration = pcm.len() as f64 / 44100.;
        let mut wav = Vec::new();
        wav.extend(b"RIFF");
        wav.extend((36 + pcm.len() as u32).to_le_bytes());
        wav.extend(b"WAVEfmt ");
        wav.extend(16u32.to_le_bytes());
        wav.extend(1u16.to_le_bytes());
        wav.extend(1u16.to_le_bytes());
        wav.extend(22050u32.to_le_bytes());
        wav.extend(44100u32.to_le_bytes());
        wav.extend(2u16.to_le_bytes());
        wav.extend(16u16.to_le_bytes());
        wav.extend(b"data");
        wav.extend((pcm.len() as u32).to_le_bytes());
        wav.extend(pcm);
        fs::write(root.join(format!("music/{track:02}.wav")), wav)?;
        catalog.push(serde_json::json!({"track":track,"name":name,"seconds":pcm_duration,"ticks":ticks,"loop_detected":loop_start.is_some()}));
        println!("Musique {track:02}: {name} ({ticks} ticks)");
    }
    let catalog_path = root.join("music/catalog.json");
    if only.is_some() && catalog_path.exists() {
        let mut previous: Vec<serde_json::Value> =
            serde_json::from_slice(&fs::read(&catalog_path)?)?;
        for entry in catalog {
            if let Some(existing) = previous.iter_mut().find(|v| v["track"] == entry["track"]) {
                *existing = entry;
            } else {
                previous.push(entry);
            }
        }
        previous.sort_by_key(|v| v["track"].as_u64());
        fs::write(catalog_path, serde_json::to_vec_pretty(&previous)?)?;
    } else {
        fs::write(catalog_path, serde_json::to_vec_pretty(&catalog)?)?;
    }
    Ok(())
}
