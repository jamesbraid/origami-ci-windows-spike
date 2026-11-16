//! D3D12 section import, copied unchanged from spike/windows/dxprobe/src/main.rs (the earlier
//! native-windows spike) except that the probe `main` is gone and the frame constants are pub.
//! A pagefile-backed section stands in for the QEMU SHM_WIN32 region; Rig imports it as a D3D12
//! heap and copies it on the GPU into a composition swap chain, with no CPU copy of the picture.
#![allow(dead_code)]

use std::mem::ManuallyDrop;
use std::time::Instant;
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::DirectComposition::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::System::LibraryLoader::*;
use windows::Win32::System::Memory::*;
use windows::Win32::System::Threading::*;
use windows::Win32::UI::WindowsAndMessaging::*;

pub const W: u32 = 1280;
pub const H: u32 = 1024;
pub const PITCH: u32 = W * 4; // 5120, a multiple of 256
pub const SIZE: usize = (PITCH * H) as usize; // 5,242,880 = 80 * 64 KiB

pub fn say(k: &str, v: impl std::fmt::Display) {
    println!("{k}={v}");
}

pub fn barrier(res: &ID3D12Resource, from: D3D12_RESOURCE_STATES, to: D3D12_RESOURCE_STATES) -> D3D12_RESOURCE_BARRIER {
    D3D12_RESOURCE_BARRIER {
        Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
        Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
        Anonymous: D3D12_RESOURCE_BARRIER_0 {
            Transition: ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
                pResource: ManuallyDrop::new(Some(res.clone())),
                Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                StateBefore: from,
                StateAfter: to,
            }),
        },
    }
}

fn footprint_loc(res: &ID3D12Resource) -> D3D12_TEXTURE_COPY_LOCATION {
    D3D12_TEXTURE_COPY_LOCATION {
        pResource: ManuallyDrop::new(Some(res.clone())),
        Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
        Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
            PlacedFootprint: D3D12_PLACED_SUBRESOURCE_FOOTPRINT {
                Offset: 0,
                Footprint: D3D12_SUBRESOURCE_FOOTPRINT {
                    Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    Width: W,
                    Height: H,
                    Depth: 1,
                    RowPitch: PITCH,
                },
            },
        },
    }
}

fn tex_loc(res: &ID3D12Resource) -> D3D12_TEXTURE_COPY_LOCATION {
    D3D12_TEXTURE_COPY_LOCATION {
        pResource: ManuallyDrop::new(Some(res.clone())),
        Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
        Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 { SubresourceIndex: 0 },
    }
}

fn buffer_desc_f(size: u64, flags: D3D12_RESOURCE_FLAGS) -> D3D12_RESOURCE_DESC {
    D3D12_RESOURCE_DESC {
        Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
        Alignment: 0,
        Width: size,
        Height: 1,
        DepthOrArraySize: 1,
        MipLevels: 1,
        Format: DXGI_FORMAT_UNKNOWN,
        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
        Flags: flags,
    }
}

fn buffer_desc(size: u64) -> D3D12_RESOURCE_DESC {
    buffer_desc_f(size, D3D12_RESOURCE_FLAG_NONE)
}

pub struct Gpu {
    dev: ID3D12Device,
    queue: ID3D12CommandQueue,
    alloc: ID3D12CommandAllocator,
    list: ID3D12GraphicsCommandList,
    fence: ID3D12Fence,
    event: HANDLE,
    value: u64,
}

impl Gpu {
    fn wait(&mut self) -> Result<()> {
        self.value += 1;
        unsafe {
            self.queue.Signal(&self.fence, self.value)?;
            if self.fence.GetCompletedValue() < self.value {
                self.fence.SetEventOnCompletion(self.value, self.event)?;
                WaitForSingleObject(self.event, 10_000);
            }
        }
        Ok(())
    }
    fn begin(&self) -> Result<()> {
        unsafe {
            self.alloc.Reset()?;
            self.list.Reset(&self.alloc, None)
        }
    }
    fn submit(&mut self) -> Result<()> {
        unsafe {
            self.list.Close()?;
            let l: ID3D12CommandList = self.list.cast()?;
            self.queue.ExecuteCommandLists(&[Some(l)]);
        }
        self.wait()
    }
}

fn readback_buffer(dev: &ID3D12Device) -> Result<ID3D12Resource> {
    let props = D3D12_HEAP_PROPERTIES { Type: D3D12_HEAP_TYPE_READBACK, ..Default::default() };
    let mut rb: Option<ID3D12Resource> = None;
    unsafe {
        dev.CreateCommittedResource(&props, D3D12_HEAP_FLAG_NONE, &buffer_desc(SIZE as u64),
            D3D12_RESOURCE_STATE_COPY_DEST, None, &mut rb)?;
    }
    Ok(rb.unwrap())
}

fn compare(rb: &ID3D12Resource, src: *const u8) -> Result<usize> {
    let mut p: *mut std::ffi::c_void = std::ptr::null_mut();
    unsafe {
        rb.Map(0, None, Some(&mut p))?;
        let a = std::slice::from_raw_parts(p as *const u8, SIZE);
        let b = std::slice::from_raw_parts(src, SIZE);
        let bad = a.chunks(4).zip(b.chunks(4)).filter(|(x, y)| x[..3] != y[..3]).count();
        rb.Unmap(0, None);
        Ok(bad)
    }
}

fn vulkan_probe() {
    unsafe {
        let lib = match LoadLibraryA(s!("vulkan-1.dll")) {
            Ok(l) => l,
            Err(e) => return say("vulkan_loader", format!("absent ({e})")),
        };
        say("vulkan_loader", "present");
        type CreateInst = unsafe extern "system" fn(*const u32, *const u8, *mut usize) -> i32;
        type Enum = unsafe extern "system" fn(usize, *mut u32, *mut usize) -> i32;
        let ci = GetProcAddress(lib, s!("vkCreateInstance"));
        let en = GetProcAddress(lib, s!("vkEnumeratePhysicalDevices"));
        let (Some(ci), Some(en)) = (ci, en) else { return say("vulkan_devices", "no entry points") };
        let ci: CreateInst = std::mem::transmute(ci);
        let en: Enum = std::mem::transmute(en);
        let info = [1u32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]; // sType=INSTANCE_CREATE_INFO, rest zero
        let mut inst = 0usize;
        let r = ci(info.as_ptr(), std::ptr::null(), &mut inst);
        if r != 0 { return say("vulkan_devices", format!("vkCreateInstance={r}")); }
        let mut n = 0u32;
        let r = en(inst, &mut n, std::ptr::null_mut());
        say("vulkan_devices", format!("vkEnumeratePhysicalDevices={r} count={n}"));
    }
}

pub struct Rig {
    pub factory: IDXGIFactory4,
    pub gpu: Gpu,
    pub buf: ID3D12Resource,
    pub tex: ID3D12Resource,
    pub rb: ID3D12Resource,
    pub mem: *mut u8,
    pub verify_bytes: u64,
    _heap: ID3D12Heap,
}

impl Rig {
    /// Stand in for QEMU (pagefile section), import it into D3D12 and prove the pixels.
    pub fn new(warp: bool) -> Result<Rig> {
        unsafe {
            let section = CreateFileMappingW(INVALID_HANDLE_VALUE, None, PAGE_READWRITE, 0, SIZE as u32, PCWSTR::null())?;
            let view = MapViewOfFile(section, FILE_MAP_ALL_ACCESS, 0, 0, 0);
            let mem = view.Value as *mut u8;
            for (i, c) in std::slice::from_raw_parts_mut(mem, SIZE).chunks_mut(4).enumerate() {
                let (x, y) = ((i as u32) % W, (i as u32) / W);
                c.copy_from_slice(&[(x & 255) as u8, (y & 255) as u8, ((x ^ y) & 255) as u8, 255]);
            }
            say("section_bytes", SIZE);
            let factory: IDXGIFactory4 = CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0))?;
            let adapter: IDXGIAdapter = if warp { factory.EnumWarpAdapter::<IDXGIAdapter>()? } else { factory.EnumAdapters(0)? };
            let desc = adapter.GetDesc()?;
            say("adapter", String::from_utf16_lossy(&desc.Description).trim_end_matches('\0'));
            let mut dev: Option<ID3D12Device> = None;
            D3D12CreateDevice(&adapter, D3D_FEATURE_LEVEL_11_0, &mut dev)?;
            let dev = dev.unwrap();
            let mut eh = D3D12_FEATURE_DATA_EXISTING_HEAPS::default();
            let r = dev.CheckFeatureSupport(D3D12_FEATURE_EXISTING_HEAPS, &mut eh as *mut _ as *mut _,
                std::mem::size_of_val(&eh) as u32);
            say("existing_heaps_supported", format!("{:?} supported={}", r.is_ok(), eh.Supported.as_bool()));
            let dev3: ID3D12Device3 = dev.cast()?;
            let heap: ID3D12Heap = dev3.OpenExistingHeapFromFileMapping(section)
                .map_err(|e| { say("open_heap_from_file_mapping", format!("FAILED {e}")); e })?;
            let hd = heap.GetDesc();
            say("open_heap_from_file_mapping", format!("ok size={} type={:?} cpu_page={:?} pool={:?} flags={:#x}",
                hd.SizeInBytes, hd.Properties.Type, hd.Properties.CPUPageProperty, hd.Properties.MemoryPoolPreference, hd.Flags.0));
            let mut buf: Option<ID3D12Resource> = None;
            for (name, fl) in [("none", D3D12_RESOURCE_FLAG_NONE), ("allow_cross_adapter", D3D12_RESOURCE_FLAG_ALLOW_CROSS_ADAPTER)] {
                let mut b: Option<ID3D12Resource> = None;
                match dev.CreatePlacedResource(&heap, 0, &buffer_desc_f(SIZE as u64, fl), D3D12_RESOURCE_STATE_COMMON, None, &mut b) {
                    Ok(()) => { say("placed_buffer_flags", format!("{name}: ok")); if buf.is_none() { buf = b; } }
                    Err(e) => say("placed_buffer_flags", format!("{name}: FAILED {e}")),
                }
            }
            let buf = buf.ok_or_else(|| Error::new(E_FAIL, "no placed buffer"))?;
            let queue: ID3D12CommandQueue = dev.CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC {
                Type: D3D12_COMMAND_LIST_TYPE_DIRECT, ..Default::default() })?;
            let alloc: ID3D12CommandAllocator = dev.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
            let list: ID3D12GraphicsCommandList = dev.CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, &alloc, None)?;
            list.Close()?;
            let fence: ID3D12Fence = dev.CreateFence(0, D3D12_FENCE_FLAG_NONE)?;
            let event = CreateEventW(None, false, false, PCWSTR::null())?;
            let gpu = Gpu { dev: dev.clone(), queue, alloc, list, fence, event, value: 0 };
            let tprops = D3D12_HEAP_PROPERTIES { Type: D3D12_HEAP_TYPE_DEFAULT, ..Default::default() };
            let tdesc = D3D12_RESOURCE_DESC {
                Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D, Alignment: 0, Width: W as u64, Height: H,
                DepthOrArraySize: 1, MipLevels: 1, Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                Layout: D3D12_TEXTURE_LAYOUT_UNKNOWN, Flags: D3D12_RESOURCE_FLAG_NONE };
            let mut tex: Option<ID3D12Resource> = None;
            dev.CreateCommittedResource(&tprops, D3D12_HEAP_FLAG_NONE, &tdesc, D3D12_RESOURCE_STATE_COPY_DEST, None, &mut tex)?;
            let rb = readback_buffer(&dev)?;
            Ok(Rig { factory, gpu, buf, tex: tex.unwrap(), rb, mem, verify_bytes: 0, _heap: heap })
        }
    }

    /// One GPU copy of the section into `target` (in state `rest` before and after). With
    /// `verify`, also reads the result back (the check only, not part of the picture path)
    /// and returns the number of mismatching pixels against the section.
    pub fn frame(&mut self, target: &ID3D12Resource, rest: D3D12_RESOURCE_STATES, verify: bool) -> Result<Option<usize>> {
        unsafe {
            let g = &mut self.gpu;
            g.begin()?;
            if rest != D3D12_RESOURCE_STATE_COPY_DEST {
                g.list.ResourceBarrier(&[barrier(target, rest, D3D12_RESOURCE_STATE_COPY_DEST)]);
            }
            g.list.CopyTextureRegion(&tex_loc(target), 0, 0, 0, &footprint_loc(&self.buf), None);
            if verify {
                g.list.ResourceBarrier(&[barrier(target, D3D12_RESOURCE_STATE_COPY_DEST, D3D12_RESOURCE_STATE_COPY_SOURCE)]);
                g.list.CopyTextureRegion(&footprint_loc(&self.rb), 0, 0, 0, &tex_loc(target), None);
                g.list.ResourceBarrier(&[barrier(target, D3D12_RESOURCE_STATE_COPY_SOURCE, D3D12_RESOURCE_STATE_COPY_DEST)]);
            }
            if rest != D3D12_RESOURCE_STATE_COPY_DEST {
                g.list.ResourceBarrier(&[barrier(target, D3D12_RESOURCE_STATE_COPY_DEST, rest)]);
            }
            g.submit()?;
            if verify {
                self.verify_bytes += SIZE as u64;
                return Ok(Some(compare(&self.rb, self.mem)?));
            }
            Ok(None)
        }
    }

    /// A composition swapchain on the rig's queue, as a SwapChainPanel or DirectComposition visual wants.
    pub fn composition_swapchain(&self) -> Result<IDXGISwapChain3> {
        unsafe {
            let f2: IDXGIFactory2 = self.factory.cast()?;
            let d = DXGI_SWAP_CHAIN_DESC1 {
                Width: W, Height: H, Format: DXGI_FORMAT_B8G8R8A8_UNORM, Stereo: false.into(),
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT, BufferCount: 2,
                Scaling: DXGI_SCALING_STRETCH, SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
                AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED, Flags: 0 };
            f2.CreateSwapChainForComposition(&self.gpu.queue, &d, None)?.cast()
        }
    }

    pub fn present_frame(&mut self, sc: &IDXGISwapChain3, verify: bool) -> Result<Option<usize>> {
        unsafe {
            let bb: ID3D12Resource = sc.GetBuffer(sc.GetCurrentBackBufferIndex())?;
            let r = self.frame(&bb, D3D12_RESOURCE_STATE_PRESENT, verify)?;
            sc.Present(0, DXGI_PRESENT(0)).ok()?;
            Ok(r)
        }
    }
}

