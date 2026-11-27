//! Fast triple-buffered `IOSurface` + `CALayer` presenter for macOS.
//!
//! `CALayer.contents` accepts an `IOSurface` directly, allowing WindowServer to
//! composite the frame straight from unified GPU memory with zero `CGImage`
//! creation, zero CoreAnimation CPU copy/swizzle, and zero per-frame allocations.

#![cfg(all(feature = "cpu", target_os = "macos"))]

use std::ffi::c_void;
use std::ptr;

use objc2::msg_send;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool};
use objc2_quartz_core::{CALayer, CATransaction, kCAGravityTopLeft};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

const K_CF_NUMBER_SINT32_TYPE: isize = 3;
const K_PIXEL_FORMAT_BGRA: i32 = u32::from_be_bytes(*b"BGRA") as i32;

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
  static kCFTypeDictionaryKeyCallBacks: c_void;
  static kCFTypeDictionaryValueCallBacks: c_void;
  fn CFNumberCreate(allocator: *const c_void, the_type: isize, value_ptr: *const c_void) -> *const c_void;
  fn CFDictionaryCreate(
    allocator: *const c_void,
    keys: *const *const c_void,
    values: *const *const c_void,
    num_values: isize,
    key_callbacks: *const c_void,
    value_callbacks: *const c_void,
  ) -> *const c_void;
  fn CFRelease(cf: *const c_void);
}

#[link(name = "IOSurface", kind = "framework")]
unsafe extern "C" {
  static kIOSurfaceWidth: *const c_void;
  static kIOSurfaceHeight: *const c_void;
  static kIOSurfaceBytesPerElement: *const c_void;
  static kIOSurfaceBytesPerRow: *const c_void;
  static kIOSurfaceAllocSize: *const c_void;
  static kIOSurfacePixelFormat: *const c_void;

  fn IOSurfaceCreate(properties: *const c_void) -> *mut c_void;
  fn IOSurfaceLock(buffer: *mut c_void, options: u32, seed: *mut u32) -> i32;
  fn IOSurfaceUnlock(buffer: *mut c_void, options: u32, seed: *mut u32) -> i32;
  fn IOSurfaceGetBaseAddress(buffer: *mut c_void) -> *mut c_void;
  fn IOSurfaceGetBytesPerRow(buffer: *mut c_void) -> usize;
  fn IOSurfaceGetAllocSize(buffer: *mut c_void) -> usize;
  fn IOSurfaceIsInUse(buffer: *mut c_void) -> u8;
}

struct IoSurface {
  raw: *mut c_void,
  bytes_per_row: usize,
  alloc_size: usize,
}

impl IoSurface {
  fn new_bgra(width: usize, height: usize) -> Option<Self> {
    let w = i32::try_from(width).ok()?;
    let h = i32::try_from(height).ok()?;
    let bpe: i32 = 4;
    let bpr_usize = (width * 4 + 63) & !63;
    let bpr = i32::try_from(bpr_usize).ok()?;
    let alloc = i32::try_from(bpr_usize.checked_mul(height)?).ok()?;
    let fmt = K_PIXEL_FORMAT_BGRA;

    unsafe {
      let n_w = CFNumberCreate(ptr::null(), K_CF_NUMBER_SINT32_TYPE, (&w as *const i32).cast());
      let n_h = CFNumberCreate(ptr::null(), K_CF_NUMBER_SINT32_TYPE, (&h as *const i32).cast());
      let n_bpe = CFNumberCreate(ptr::null(), K_CF_NUMBER_SINT32_TYPE, (&bpe as *const i32).cast());
      let n_bpr = CFNumberCreate(ptr::null(), K_CF_NUMBER_SINT32_TYPE, (&bpr as *const i32).cast());
      let n_alloc =
        CFNumberCreate(ptr::null(), K_CF_NUMBER_SINT32_TYPE, (&alloc as *const i32).cast());
      let n_fmt = CFNumberCreate(ptr::null(), K_CF_NUMBER_SINT32_TYPE, (&fmt as *const i32).cast());

      let keys = [
        kIOSurfaceWidth,
        kIOSurfaceHeight,
        kIOSurfaceBytesPerElement,
        kIOSurfaceBytesPerRow,
        kIOSurfaceAllocSize,
        kIOSurfacePixelFormat,
      ];
      let vals = [n_w, n_h, n_bpe, n_bpr, n_alloc, n_fmt];

      let dict = CFDictionaryCreate(
        ptr::null(),
        keys.as_ptr(),
        vals.as_ptr(),
        keys.len() as isize,
        &raw const kCFTypeDictionaryKeyCallBacks,
        &raw const kCFTypeDictionaryValueCallBacks,
      );

      for v in vals {
        if !v.is_null() {
          CFRelease(v);
        }
      }
      if dict.is_null() {
        return None;
      }

      let raw = IOSurfaceCreate(dict);
      CFRelease(dict);
      if raw.is_null() {
        return None;
      }
      let bytes_per_row = IOSurfaceGetBytesPerRow(raw);
      let alloc_size = IOSurfaceGetAllocSize(raw);
      Some(IoSurface { raw, bytes_per_row, alloc_size })
    }
  }

  fn is_in_use(&self) -> bool {
    unsafe { IOSurfaceIsInUse(self.raw) != 0 }
  }

  fn write_pixels(&self, fill: impl FnOnce(&mut [u8], usize)) {
    unsafe {
      if IOSurfaceLock(self.raw, 0, ptr::null_mut()) != 0 {
        return;
      }
      let base = IOSurfaceGetBaseAddress(self.raw).cast::<u8>();
      if !base.is_null() && self.alloc_size > 0 {
        let slice = std::slice::from_raw_parts_mut(base, self.alloc_size);
        fill(slice, self.bytes_per_row);
      }
      IOSurfaceUnlock(self.raw, 0, ptr::null_mut());
    }
  }

  fn as_any_object(&self) -> &AnyObject {
    // SAFETY: `IOSurfaceRef` is toll-free bridged to `IOSurface` (Objective-C object).
    unsafe { &*self.raw.cast::<AnyObject>() }
  }
}

impl Drop for IoSurface {
  fn drop(&mut self) {
    if !self.raw.is_null() {
      unsafe { CFRelease(self.raw.cast()) };
    }
  }
}

pub(crate) struct MacPresenter {
  layer: Retained<CALayer>,
  surfaces: Vec<IoSurface>,
  size: (usize, usize),
  last_presented: Option<usize>,
  last_scale: f64,
}

impl MacPresenter {
  const MAX_SURFACES: usize = 4;

  pub(crate) fn new(window: &winit::window::Window) -> Result<Self, String> {
    let handle = window.window_handle().map_err(|e| e.to_string())?;
    let RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
      return Err("expected AppKit window handle on macOS".to_string());
    };
    let ns_view: *mut AnyObject = appkit.ns_view.as_ptr().cast();
    // Create a layer-hosting NSView (`setLayer:` before `setWantsLayer:YES`) so
    // AppKit never overwrites `layer.contents` during window redraws or resizes.
    let layer = CALayer::new();
    unsafe {
      layer.setContentsGravity(kCAGravityTopLeft);
      layer.setNeedsDisplayOnBoundsChange(false);
      layer.setOpaque(true);
      let _: () = msg_send![ns_view, setLayer: &*layer];
      let _: () = msg_send![ns_view, setWantsLayer: Bool::YES];
    }
    Ok(MacPresenter {
      layer,
      surfaces: Vec::with_capacity(Self::MAX_SURFACES),
      size: (0, 0),
      last_presented: None,
      last_scale: 0.0,
    })
  }

  fn acquire_surface(&mut self, width: usize, height: usize) -> Option<usize> {
    if self.size != (width, height) {
      self.surfaces.clear();
      self.size = (width, height);
      self.last_presented = None;
    }

    // Prefer a free surface different from the one currently bound to `layer.contents`
    // so CoreAnimation always sees a pointer change and latches the new frame without flicker.
    if let Some(idx) = self
      .surfaces
      .iter()
      .enumerate()
      .position(|(i, s)| Some(i) != self.last_presented && !s.is_in_use())
    {
      return Some(idx);
    }

    if self.surfaces.len() < Self::MAX_SURFACES {
      let surface = IoSurface::new_bgra(width, height)?;
      self.surfaces.push(surface);
      return Some(self.surfaces.len() - 1);
    }

    // All surfaces in use: pick any surface different from `last_presented` (round-robin).
    let next = self.last_presented.map_or(0, |i| (i + 1) % self.surfaces.len());
    Some(next)
  }

  /// Locks a pooled `BGRA8` `IOSurface`, lets `fill(buf, bytes_per_row)` write directly into
  /// unified GPU memory, and assigns it to `CALayer.contents` without `CGImage` conversion.
  pub(crate) fn present_bgra8(
    &mut self,
    width: usize,
    height: usize,
    scale: f64,
    fill: impl FnOnce(&mut [u8], usize),
  ) {
    if width == 0 || height == 0 {
      return;
    }
    let Some(idx) = self.acquire_surface(width, height) else {
      return;
    };
    let surface = &self.surfaces[idx];
    surface.write_pixels(fill);

    CATransaction::begin();
    CATransaction::setDisableActions(true);
    if (scale - self.last_scale).abs() > 1e-4 {
      self.layer.setContentsScale(scale);
      self.last_scale = scale;
    }
    unsafe { self.layer.setContents(Some(surface.as_any_object())) };
    CATransaction::commit();
    self.last_presented = Some(idx);
  }
}
