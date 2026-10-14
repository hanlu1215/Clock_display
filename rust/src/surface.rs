//! ===========================================================================
//!  一块 32bpp 预乘 alpha 的内存画布，外加一个 GDI+ 画布（对应 C 版 surface.c）。
//!
//!  UpdateLayeredWindow(ULW_ALPHA) 要的就是这个格式：逐像素透明度。
//!  文字的抗锯齿边缘是按 alpha 混合上去的，所以不会有「色键抠图」那种
//!  一圈杂色的锯齿边 —— 这正是要透明背景时钟必须走这条路的原因。
//! ========================================================================

use std::ptr;

use windows::Win32::Foundation::{COLORREF, HWND, POINT, SIZE};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, SelectObject, BITMAPINFO,
    BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION, DIB_RGB_COLORS, HBITMAP, HDC, HGDIOBJ,
};
use windows::Win32::Graphics::GdiPlus::{
    FlushIntentionSync, GdipCreateBitmapFromScan0, GdipDeleteGraphics, GdipDisposeImage,
    GdipFlush, GdipGetImageGraphicsContext, GdipGraphicsClear, GpBitmap, GpGraphics, GpImage,
    Status,
};
use windows::Win32::UI::WindowsAndMessaging::{UpdateLayeredWindow, ULW_ALPHA};

/// PixelFormat32bppPARGB = 11 | (32<<8) | Alpha | PAlpha | GDI = 0x000E200B
/// （GDI+ 问世以来没动过的 ABI 常量；windows crate 没收录这个宏，照着
/// gdipluspixelformats.h 硬编码。）
const PIXEL_FORMAT_32BPP_PARGB: i32 = 0x000E200B;

/// GpStatus == 0 即成功。windows crate 里这个常量名字就叫 `Ok`，
/// 引进来会和 Result 的 Ok 打架，所以直接比数值。
fn gp_ok(status: Status) -> bool {
    status.0 == 0
}

pub struct Surface {
    pub width: i32,
    pub height: i32,
    dc: HDC,                     // 内存 DC，位图已选入
    dib: HBITMAP,                // 32bpp 自上而下的 DIB
    old_bitmap: HGDIOBJ,
    pub bits: *mut u8,           // DIB 的像素：每像素 BGRA，alpha 已预乘，行距 width*4
    bitmap: *mut GpBitmap,       // 把 bits 直接包成 GDI+ 位图，画完即所见
    pub graphics: *mut GpGraphics,
}

impl Surface {
    pub fn new() -> Surface {
        Surface {
            width: 0,
            height: 0,
            dc: HDC::default(),
            dib: HBITMAP::default(),
            old_bitmap: HGDIOBJ::default(),
            bits: ptr::null_mut(),
            bitmap: ptr::null_mut(),
            graphics: ptr::null_mut(),
        }
    }

    /// 尺寸没变就复用（内容不保证保留，重新画之前一律先 clear）。
    pub fn ensure(&mut self, width: i32, height: i32) -> bool {
        if width <= 0 || height <= 0 {
            return false;
        }
        if !self.graphics.is_null() && width == self.width && height == self.height {
            return true;
        }

        self.release();

        unsafe {
            let mut info = BITMAPINFO::default();
            info.bmiHeader = BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height, // 负数 = 自上而下，行序和 GDI+ 一致
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            };

            let mut bits: *mut std::ffi::c_void = ptr::null_mut();
            let dib = match CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0) {
                Ok(dib) => dib,
                Err(_) => return false,
            };
            if bits.is_null() {
                let _ = DeleteObject(dib.into());
                return false;
            }

            let dc = CreateCompatibleDC(None);
            if dc.is_invalid() {
                let _ = DeleteObject(dib.into());
                return false;
            }

            // PixelFormat32bppPARGB：预乘 alpha，正好是 ULW_ALPHA 要的格式。
            // 这里把 DIB 的内存直接交给 GDI+ 当画布，所以画完 Flush 之后，
            // bits 里就是最终像素 —— 量墨迹、推给窗口都直接读它。
            let mut bitmap: *mut GpBitmap = ptr::null_mut();
            if !gp_ok(GdipCreateBitmapFromScan0(
                width,
                height,
                width * 4,
                PIXEL_FORMAT_32BPP_PARGB,
                Some(bits.cast()),
                &mut bitmap,
            )) {
                let _ = DeleteDC(dc);
                let _ = DeleteObject(dib.into());
                return false;
            }

            let mut graphics: *mut GpGraphics = ptr::null_mut();
            if !gp_ok(GdipGetImageGraphicsContext(bitmap.cast::<GpImage>(), &mut graphics)) {
                let _ = GdipDisposeImage(bitmap.cast::<GpImage>());
                let _ = DeleteDC(dc);
                let _ = DeleteObject(dib.into());
                return false;
            }

            self.width = width;
            self.height = height;
            self.dib = dib;
            self.dc = dc;
            self.bits = bits.cast();
            self.bitmap = bitmap;
            self.graphics = graphics;
            self.old_bitmap = SelectObject(dc, dib.into());
            true
        }
    }

    /// 释放顺序要紧：GDI+ 对象先没，再还 GDI 位图，最后删 DC。
    pub fn release(&mut self) {
        unsafe {
            if !self.graphics.is_null() {
                let _ = GdipDeleteGraphics(self.graphics);
                self.graphics = ptr::null_mut();
            }
            if !self.bitmap.is_null() {
                let _ = GdipDisposeImage(self.bitmap.cast::<GpImage>());
                self.bitmap = ptr::null_mut();
            }
            if !self.dc.is_invalid() && !self.old_bitmap.is_invalid() {
                SelectObject(self.dc, self.old_bitmap);
            }
            self.old_bitmap = HGDIOBJ::default();
            if !self.dib.is_invalid() {
                let _ = DeleteObject(self.dib.into());
                self.dib = HBITMAP::default();
            }
            if !self.dc.is_invalid() {
                let _ = DeleteDC(self.dc);
                self.dc = HDC::default();
            }
        }
        self.bits = ptr::null_mut();
        self.width = 0;
        self.height = 0;
    }

    /// 整块清成全透明。
    pub fn clear(&self) {
        unsafe {
            if !self.graphics.is_null() {
                let _ = GdipGraphicsClear(self.graphics, 0x00000000);
            }
        }
    }

    /// 等 GDI+ 把这一批绘制真正落到 bits 上。
    /// 想直接读 bits（比如量文字墨迹）之前必须先调它。
    pub fn flush(&self) {
        unsafe {
            if !self.graphics.is_null() {
                let _ = GdipFlush(self.graphics, FlushIntentionSync);
            }
        }
    }

    /// 把画布内容整体推给分层窗口；位置不传，只更新内容。
    /// 传无效窗口或者尺寸对不上都返回 false。
    pub fn present(&self, window: HWND) -> bool {
        if window.is_invalid() || self.dc.is_invalid() {
            return false;
        }

        let source = POINT { x: 0, y: 0 };
        let size = SIZE {
            cx: self.width,
            cy: self.height,
        };
        let blend = BLENDFUNCTION {
            BlendOp: 0, // AC_SRC_OVER
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: 1, // AC_SRC_ALPHA
        };

        unsafe {
            // pptDst 传 None = 位置不动，只换内容。窗口在哪儿由 SetWindowPos 管，
            // 拖动的时候就不用每秒重推一次整块位图了。
            UpdateLayeredWindow(
                window,
                None,
                None,
                Some(&size),
                Some(self.dc),
                Some(&source),
                COLORREF(0),
                Some(&blend),
                ULW_ALPHA,
            )
            .is_ok()
        }
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        self.release();
    }
}
