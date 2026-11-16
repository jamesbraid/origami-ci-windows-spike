//! Stand-in for the Rust core: owns the display memory, receives input, and presents to a
//! SwapChainPanel. The real core would do the same calls with QEMU's section instead of ours.
use crate::d3d::{self, Rig, H, SIZE, W};
use std::cell::RefCell;
use std::rc::Rc;
use wc::Interface as _;
use windows::core::Interface as _;
use windows_reactor::*;

/// What the core has been told. The shell prints every line to stdout so a CI log shows that
/// input arrived in the core, not just in the UI layer.
#[derive(Default)]
pub struct Core {
    pub grabbed: bool,
    pub keys: u32,
    pub chars: String,
    pub pointer: Option<(f64, f64)>,
    pub buttons: u32,
    pub frame: u32,
    pub last: String,
    pub panel: (f64, f64),
}

pub type SharedCore = Rc<RefCell<Core>>;

impl Core {
    pub fn say(&mut self, what: String) {
        println!("core: {what}");
        self.last = what;
    }

    pub fn key(&mut self, info: &KeyEventInfo) {
        self.keys += 1;
        let m = info.modifiers;
        self.say(format!(
            "key vk={:#x} ctrl={} shift={} alt={}",
            info.key.0,
            m.contains(InputModifiers::CONTROL),
            m.contains(InputModifiers::SHIFT),
            m.contains(InputModifiers::ALT)
        ));
    }

    pub fn character(&mut self, c: u16) {
        if let Some(ch) = char::from_u32(c as u32) {
            self.chars.push(ch);
            self.say(format!("char {ch:?}"));
        }
    }

    pub fn pointer(&mut self, kind: &str, info: &PointerEventInfo) {
        self.pointer = Some((info.x, info.y));
        self.buttons = info.is_left_button_pressed as u32;
        self.say(format!(
            "pointer {kind} x={:.0} y={:.0} left={} captured={:?}",
            info.x, info.y, info.is_left_button_pressed, info.capture_succeeded
        ));
    }
}

/// One swap chain bound to one panel, drawn from one section.
pub struct Surface {
    rig: Rig,
    sc: windows::Win32::Graphics::Dxgi::IDXGISwapChain3,
    unk: wc::IUnknown,
    pub tag: u32,
}

impl Surface {
    /// Hand the same swap chain to a (possibly new) panel. The page that hosts the panel is
    /// rebuilt when the user switches sidebar pages, so a binding made once goes stale.
    pub fn rebind(&self, panel: &ElementRef<SwapChainPanel>, done: impl Fn(Result<(), String>) + 'static) {
        let queued = panel.request_set_swap_chain(self.unk.clone(), move |r| done(r.map_err(|e| format!("{e:?}"))));
        d3d::say("panel_rebind_queued", queued);
    }
}

impl Surface {
    /// Paint into the section, then let the GPU copy it to the back buffer and present.
    pub fn frame(&mut self, core: &Core, console: bool) {
        let t = core.frame;
        // SAFETY: rig.mem is the live mapping of SIZE bytes created in Rig::new.
        let px = unsafe { std::slice::from_raw_parts_mut(self.rig.mem, SIZE) };
        if console {
            // A terminal-looking grid: dark background, a row of "text" cells per line, and a
            // cursor that moves with each typed character.
            let cols = 80;
            let cell_w = (W as usize) / cols;
            let cell_h = 16;
            for (i, c) in px.chunks_mut(4).enumerate() {
                let (x, y) = (i % W as usize, i / W as usize);
                let (cx, cy) = (x / cell_w, y / cell_h);
                let on = (cx * 7 + cy * 13) % 5 < 2 && (x % cell_w) > 1 && (y % cell_h) > 3 && (y % cell_h) < 13;
                let v = if on { 0xc8 } else { 0x1c };
                c.copy_from_slice(&[v, v, v, 255]);
            }
            let n = core.chars.chars().count();
            let (cx, cy) = (n % cols, n / cols);
            for y in cy * cell_h..(cy * cell_h + cell_h).min(H as usize) {
                for x in cx * cell_w..(cx * cell_w + cell_w).min(W as usize) {
                    px[(y * W as usize + x) * 4..][..4].copy_from_slice(&[0x40, 0xd0, 0x40, 255]);
                }
            }
        } else {
            // Animated gradient, a band that scrolls, and a marker square at the pointer.
            for (i, c) in px.chunks_mut(4).enumerate() {
                let (x, y) = ((i as u32) % W, (i as u32) / W);
                let band = ((y + t * 6) / 64) % 2 == 0;
                let g = if band { 0x30 } else { 0x00 };
                c.copy_from_slice(&[((x / 5) & 255) as u8, ((y / 4 + g) & 255) as u8, 0x60, 255]);
            }
            if let Some((px_, py_)) = core.pointer {
                let (pw, ph) = core.panel;
                if pw > 0.0 && ph > 0.0 {
                    let mx = ((px_ / pw).clamp(0.0, 1.0) * (W - 1) as f64) as i64;
                    let my = ((py_ / ph).clamp(0.0, 1.0) * (H - 1) as f64) as i64;
                    let col: [u8; 4] = if core.grabbed { [0x20, 0xff, 0xff, 255] } else { [0xff, 0xff, 0xff, 255] };
                    for y in (my - 12).max(0)..(my + 12).min(H as i64) {
                        for x in (mx - 12).max(0)..(mx + 12).min(W as i64) {
                            px[((y as usize) * W as usize + x as usize) * 4..][..4].copy_from_slice(&col);
                        }
                    }
                }
            }
        }
        if let Err(e) = self.rig.present_frame(&self.sc, false) {
            d3d::say("present_failed", e);
        }
    }
}

/// Bind a fresh D3D12 rig to the panel. `done` runs on the UI thread once the panel took the
/// swap chain.
pub fn attach(
    panel: &ElementRef<SwapChainPanel>,
    tag: u32,
    done: impl Fn(Result<Rc<RefCell<Surface>>, String>) + 'static,
) {
    let build = || -> windows::core::Result<(Rig, windows::Win32::Graphics::Dxgi::IDXGISwapChain3, wc::IUnknown)> {
        let rig = Rig::new(true)?;
        let sc = rig.composition_swapchain()?;
        let unk: windows::core::IUnknown = sc.cast()?;
        let raw = unk.into_raw();
        // SAFETY: into_raw hands over our reference; windows-core 0.101's IUnknown takes it over.
        let iu = unsafe { wc::IUnknown::from_raw(raw) };
        Ok((rig, sc, iu))
    };
    match build() {
        Ok((rig, sc, iu)) => {
            let surface = Rc::new(RefCell::new(Surface { rig, sc, unk: iu.clone(), tag }));
            let s2 = surface.clone();
            let queued = panel.request_set_swap_chain(iu, move |r| match r {
                Ok(()) => done(Ok(s2.clone())),
                Err(e) => done(Err(format!("{e:?}"))),
            });
            d3d::say("panel_request_queued", queued);
        }
        Err(e) => done(Err(e.to_string())),
    }
}
