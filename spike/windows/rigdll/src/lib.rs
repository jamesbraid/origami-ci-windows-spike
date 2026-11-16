//! The core's side of a WinUI/C# host: a C ABI over the dxprobe rig. In the product this is the
//! UniFFI core; the surface hand-over (a raw IDXGISwapChain pointer) is what matters here.
#[path = "../../dxprobe/src/main.rs"]
#[allow(dead_code)]
mod probe;

use windows::core::Interface;

pub struct Handle {
    rig: probe::Rig,
    sc: windows::Win32::Graphics::Dxgi::IDXGISwapChain3,
}

/// Returns null and writes the failing step to stdout on error.
#[no_mangle]
pub extern "C" fn rig_create(warp: i32) -> *mut Handle {
    let r = (|| -> windows::core::Result<Handle> {
        let rig = probe::Rig::new(warp != 0)?;
        let sc = rig.composition_swapchain()?;
        Ok(Handle { rig, sc })
    })();
    match r {
        Ok(h) => Box::into_raw(Box::new(h)),
        Err(e) => { probe::say("rig_create", format!("FAILED {e}")); std::ptr::null_mut() }
    }
}

/// A new reference to the swapchain, for ISwapChainPanelNative::SetSwapChain.
#[no_mangle]
pub unsafe extern "C" fn rig_swapchain(h: *mut Handle) -> *mut core::ffi::c_void {
    (*h).sc.clone().into_raw()
}

/// Copy the shared section into the next back buffer on the GPU and present. With `verify`,
/// returns the mismatching pixel count from a readback (the check, not the picture path).
#[no_mangle]
pub unsafe extern "C" fn rig_present(h: *mut Handle, verify: i32) -> i64 {
    let h = &mut *h;
    match h.rig.present_frame(&h.sc, verify != 0) {
        Ok(Some(m)) => m as i64,
        Ok(None) => 0,
        Err(e) => { probe::say("rig_present", format!("FAILED {e}")); -1 }
    }
}
