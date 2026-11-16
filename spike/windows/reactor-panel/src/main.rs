//! Spike: Microsoft's windows-reactor (WinUI 3 from Rust) hosting the same D3D12 composition
//! swapchain as dxprobe in a XAML SwapChainPanel. Reuses dxprobe's rig.

#[path = "../../dxprobe/src/main.rs"]
#[allow(dead_code)]
mod probe;

use std::cell::RefCell;
use std::rc::Rc;
use wc100::Interface as _;
use windows::core::Interface as _;
use windows_reactor::*;

struct Host {
    panel: ElementRef<SwapChainPanel>,
}

impl Component for Host {
    type Input = ();
    type Message = ();

    fn create(_input: &(), _context: &ComponentContext<Self>) -> Self {
        Self { panel: ElementRef::new() }
    }

    fn update(&mut self, _message: (), _context: &ComponentContext<Self>) {}

    fn view(&self, _input: &(), context: &mut ViewContext<Self>) -> View {
        let panel = self.panel.clone();
        context.use_effect("surface", (), move || {
            let started = Rc::new(RefCell::new(false));
            let panel2 = panel.clone();
            let obs = panel.observe_surface(move |event| {
                if let SwapChainPanelEvent::Metrics { width, height, scale_x, .. } = event {
                    probe::say("panel_metrics", format!("{width}x{height} dip scale {scale_x}"));
                    if std::mem::replace(&mut *started.borrow_mut(), true) {
                        return;
                    }
                    if let Err(e) = attach(&panel2) {
                        probe::say("reactor_attach", format!("FAILED {e}"));
                        std::process::exit(2);
                    }
                }
            });
            Some(Box::new(move || drop(obs)))
        });
        let v: View = SwapChainPanel::new().element_ref(&self.panel).into();
        context.window_title("origami spike");
        v
    }
}

fn attach(panel: &ElementRef<SwapChainPanel>) -> windows::core::Result<()> {
    let mut rig = probe::Rig::new(true)?;
    let sc = rig.composition_swapchain()?;
    let unk: windows::core::IUnknown = sc.cast()?;
    let raw = unk.into_raw();
    // SAFETY: into_raw hands over our reference; the reactor crate's IUnknown takes it over.
    let iu = unsafe { wc100::IUnknown::from_raw(raw) };
    let sc2 = sc.clone();
    let rig = Rc::new(RefCell::new(rig));
    let queued = panel.request_set_swap_chain(iu, move |r| {
        match r {
            Ok(()) => {
                probe::say("panel_set_swap_chain", "ok");
                let mism = rig.borrow_mut().present_frame(&sc2, true);
                probe::say("panel_present_pixel_mismatches", format!("{mism:?}"));
                std::process::exit(0);
            }
            Err(e) => {
                probe::say("panel_set_swap_chain", format!("FAILED {e:?}"));
                std::process::exit(3);
            }
        }
    });
    probe::say("panel_request_queued", queued);
    Ok(())
}

fn main() {
    let r = App::run_component::<Host>(());
    probe::say("app_run_returned", format!("{r:?}"));
}
