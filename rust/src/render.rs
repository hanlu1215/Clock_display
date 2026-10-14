//! 字体与文字绘制（带描边）（对应 C 版 render.c）
//!
//! 为什么排版矩形要固定成一个「远大于文字」的尺寸，而不是用 GdipMeasureString
//! 量出来的框：
//!
//! GDI+ 量出来的框比字实际占的地方大一圈 —— 实测 Consolas 粗体 96px 下量
//! "88:88:88" 是 467×124，而真正画上去的墨迹只有 425×64。矩形一旦收紧到那个
//! 尺寸，GDI+ 居中放置的基准就开始飘：给一个 434 宽的矩形画 467 宽的排版框，
//! 整串字会往右偏二十几个像素，末尾那位数字直接被窗口切掉（踩过这个坑）。
//!
//! 固定成 4000×4000 之后，矩形的尺寸恒定，墨迹相对「矩形左上角」的偏移就是个常量，
//! 想让墨迹落在哪儿，把左上角平移同样多就行 —— 位置完全可算，不受字体和字号影响。

use std::ptr;
use std::sync::OnceLock;

use windows::core::PCWSTR;
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::GdiPlus::{
    FontStyleBold, GdipCreateFont, GdipCreateFontFamilyFromName, GdipCreateSolidFill,
    GdipCreateStringFormat, GdipDeleteBrush, GdipDeleteFont, GdipDeleteStringFormat,
    GdipDrawString, GdipSetStringFormatAlign, GdipSetStringFormatLineAlign, GpBrush, GpFont,
    GpFontFamily, GpGraphics, GpSolidFill, GpStringFormat, RectF, Status, StringAlignmentCenter,
    StringFormatFlagsNoWrap, UnitPixel,
};

use crate::config::{FONT_CHOICES, INK_ALPHA_MIN};
use crate::util::to_wide;

/// GpStatus == 0 即成功（windows crate 里这个常量叫 `Ok`，引进来会和 Result 打架，
/// 所以直接比数值）。
fn gp_ok(status: Status) -> bool {
    status.0 == 0
}

/// 排版矩形固定开这么大。
/// 调用方拿它的一半来摆位：矩形左上角 = 想让文字中心在的位置 - 这个值的一半。
pub const TEXT_RECT_SIZE: f32 = 4000.0;

// 界面字体族（按 FONT_CHOICES 顺序挑第一个装了的），取到就一直缓存着用。
// 这个 family 是「故意泄漏」的，别改成在退出时释放：
// 静态对象要等 main 返回之后才析构，那会儿 GdiplusShutdown 早跑过了，
// 再去析构一个 GDI+ 对象必崩在 CRT 的退出处理里。
// 存成 usize：0 表示「试过了，一个都没找到」。
static FONT_FAMILY: OnceLock<usize> = OnceLock::new();

pub fn family() -> Option<*mut GpFontFamily> {
    let ptr = *FONT_FAMILY.get_or_init(|| {
        for name in FONT_CHOICES {
            let wide = to_wide(name);
            let mut probe: *mut GpFontFamily = ptr::null_mut();
            unsafe {
                // 名字找不到时返回非 0 —— 正好拿来一个个试字体表里的候选。
                if gp_ok(GdipCreateFontFamilyFromName(
                    PCWSTR(wide.as_ptr()),
                    ptr::null_mut(),
                    &mut probe,
                )) && !probe.is_null()
                {
                    return probe as usize; // 就它了，一直留着
                }
            }
        }
        0
    });
    if ptr == 0 {
        None
    } else {
        Some(ptr as *mut GpFontFamily)
    }
}

/// 按「像素」字号的粗体字（不是磅值），Drop 时自动 GdipDeleteFont。
pub struct Font(*mut GpFont);

impl Font {
    pub fn create(pixel_size: f32) -> Option<Font> {
        let family = family()?;
        if pixel_size <= 0.0 {
            return None;
        }
        let mut font: *mut GpFont = ptr::null_mut();
        unsafe {
            if !gp_ok(GdipCreateFont(family, pixel_size, FontStyleBold.0, UnitPixel, &mut font)) {
                return None;
            }
        }
        Some(Font(font))
    }

    pub fn raw(&self) -> *const GpFont {
        self.0
    }
}

impl Drop for Font {
    fn drop(&mut self) {
        unsafe {
            let _ = GdipDeleteFont(self.0);
        }
    }
}

/// 画一行带描边的文字。
///
/// rect_x / rect_y 是 DrawString 那个「排版矩形」的左上角；矩形尺寸固定是
/// TEXT_RECT_SIZE 见方，文字居中排在里面，八个方向的描边就是把这个矩形
/// 平移若干像素再画一遍。
///
/// 居中的是「行框」不是墨迹：行框留着上伸部和下伸部的空档，数字既碰不到顶也
/// 踩不到底，所以看着会略偏上。墨迹的准确位置得靠 ink_bounds 量出来反推，
/// 好在矩形尺寸固定 → 矩形挪多少，墨迹就挪多少，平移量是线性的。
///
/// text 是以 0 结尾的 UTF-16（to_wide 的产物）。
pub fn render_text(
    graphics: *mut GpGraphics,
    text: &[u16],
    font: &Font,
    rect_x: f32,
    rect_y: f32,
    color: u32,
    outline_color: u32,
    outline: i32,
) {
    if graphics.is_null() || text.is_empty() || text[0] == 0 {
        return;
    }

    unsafe {
        let mut format: *mut GpStringFormat = ptr::null_mut();
        if !gp_ok(GdipCreateStringFormat(StringFormatFlagsNoWrap.0, 0, &mut format)) {
            return;
        }
        let _ = GdipSetStringFormatAlign(format, StringAlignmentCenter);
        let _ = GdipSetStringFormatLineAlign(format, StringAlignmentCenter);

        // 先把描边铺满八个方向，再把字压在上面 —— 和隐私屏幕那只钟同一个做法
        // （那边是 ±2 像素的八个偏移，这里半径按字号缩放）。
        // 同一处地方被叠上八次，抗锯齿的边就叠成了实心描边，而最外圈仍是 alpha 渐变，
        // 于是压在照片和浅色壁纸上都读得清，边缘又不显毛刺。
        if outline > 0 {
            let mut edge: *mut GpSolidFill = ptr::null_mut();
            if gp_ok(GdipCreateSolidFill(outline_color, &mut edge)) {
                let mut dy = -outline;
                while dy <= outline {
                    let mut dx = -outline;
                    while dx <= outline {
                        if !(dx == 0 && dy == 0) {
                            let rect = RectF {
                                X: rect_x + dx as f32,
                                Y: rect_y + dy as f32,
                                Width: TEXT_RECT_SIZE,
                                Height: TEXT_RECT_SIZE,
                            };
                            let _ = GdipDrawString(
                                graphics,
                                PCWSTR(text.as_ptr()),
                                -1,
                                font.raw(),
                                &rect,
                                format,
                                edge.cast::<GpBrush>(),
                            );
                        }
                        dx += outline;
                    }
                    dy += outline;
                }
                let _ = GdipDeleteBrush(edge.cast::<GpBrush>());
            }
        }

        let mut fill: *mut GpSolidFill = ptr::null_mut();
        if gp_ok(GdipCreateSolidFill(color, &mut fill)) {
            let rect = RectF {
                X: rect_x,
                Y: rect_y,
                Width: TEXT_RECT_SIZE,
                Height: TEXT_RECT_SIZE,
            };
            let _ = GdipDrawString(
                graphics,
                PCWSTR(text.as_ptr()),
                -1,
                font.raw(),
                &rect,
                format,
                fill.cast::<GpBrush>(),
            );
            let _ = GdipDeleteBrush(fill.cast::<GpBrush>());
        }

        let _ = GdipDeleteStringFormat(format);
    }
}

/// 扫一遍画布，量出真正画上去的墨迹范围（含描边）。
/// bits 是 32bpp 预乘 BGRA，alpha 不够 INK_ALPHA_MIN 的算没画。
/// 一个像素都没画到就返回 None。
pub fn ink_bounds(bits: *const u8, width: i32, height: i32) -> Option<RECT> {
    if bits.is_null() || width <= 0 || height <= 0 {
        return None;
    }

    let mut left = width;
    let mut top = height;
    let mut right = -1;
    let mut bottom = -1;

    unsafe {
        for y in 0..height {
            let row = bits.add(y as usize * width as usize * 4);
            for x in 0..width {
                if *row.add(x as usize * 4 + 3) < INK_ALPHA_MIN {
                    continue; // 第四个字节是 alpha
                }
                if x < left {
                    left = x;
                }
                if x > right {
                    right = x;
                }
                if y < top {
                    top = y;
                }
                if y > bottom {
                    bottom = y;
                }
            }
        }
    }

    if right < left || bottom < top {
        return None; // 一个像素都没画上
    }

    Some(RECT {
        left,
        top,
        right,
        bottom,
    })
}
