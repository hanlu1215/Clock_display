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
use std::sync::atomic::{AtomicUsize, Ordering};

use windows::core::PCWSTR;
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::GdiPlus::{
    FillModeAlternate, FontStyleBold, GdipCreateFont, GdipCreateFontFamilyFromName,
    GdipCreateSolidFill, GdipCreateStringFormat, GdipDeleteBrush, GdipDeleteFont,
    GdipDeleteStringFormat, GdipDrawString, GdipFillPolygon, GdipSetStringFormatAlign,
    GdipSetStringFormatLineAlign, GpBrush, GpFont, GpFontFamily, GpGraphics, GpSolidFill,
    GpStringFormat, PointF, RectF, Status, StringAlignmentCenter, StringFormatFlagsNoWrap,
    UnitPixel,
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

// 界面字体族缓存（每个候选字体一格），取到就一直缓存着用。
// 这些 family 是「故意泄漏」的，别改成在退出时释放：
// 静态对象要等 main 返回之后才析构，那会儿 GdiplusShutdown 早跑过了，
// 再去析构一个 GDI+ 对象必崩在 CRT 的退出处理里。
// 存成 usize：0 = 还没试过，1 = 试过了这台机器没有，其他 = family 指针。
static FAMILY_CACHE: [AtomicUsize; FONT_CHOICES.len()] =
    [const { AtomicUsize::new(0) }; FONT_CHOICES.len()];

/// 取第 index 个候选字体的 family，没有这台机器就返回 None。
fn family_for(index: usize) -> Option<*mut GpFontFamily> {
    let cached = FAMILY_CACHE[index].load(Ordering::Relaxed);
    if cached != 0 {
        return if cached == 1 {
            None
        } else {
            Some(cached as *mut GpFontFamily)
        };
    }

    let wide = to_wide(FONT_CHOICES[index]);
    let mut probe: *mut GpFontFamily = ptr::null_mut();
    unsafe {
        // 名字找不到时返回非 0 —— 正好拿来判断这台机器装没装。
        if gp_ok(GdipCreateFontFamilyFromName(
            PCWSTR(wide.as_ptr()),
            ptr::null_mut(),
            &mut probe,
        )) && !probe.is_null()
        {
            FAMILY_CACHE[index].store(probe as usize, Ordering::Relaxed);
            Some(probe)
        } else {
            FAMILY_CACHE[index].store(1, Ordering::Relaxed);
            None
        }
    }
}

/// 解析出实际要用的字体：preferred 是用户在菜单里选的下标（-1 = 自动）。
/// 指定的字体这台机器上没有时，回落到「从上往下挑第一个装了的」。
/// 返回 (实际生效的下标, family)，给菜单打勾用。
pub fn resolve_family(preferred: i32) -> Option<(usize, *mut GpFontFamily)> {
    if preferred >= 0 && (preferred as usize) < FONT_CHOICES.len() {
        if let Some(family) = family_for(preferred as usize) {
            return Some((preferred as usize, family));
        }
    }
    for index in 0..FONT_CHOICES.len() {
        if let Some(family) = family_for(index) {
            return Some((index, family));
        }
    }
    None
}

/// 当前实际生效的字体下标（菜单「字体」里打勾用）。
pub fn effective_font_index(preferred: i32) -> Option<usize> {
    resolve_family(preferred).map(|(index, _)| index)
}

/// 按「像素」字号的粗体字（不是磅值），Drop 时自动 GdipDeleteFont。
pub struct Font(*mut GpFont);

impl Font {
    pub fn create(pixel_size: f32, font_index: i32) -> Option<Font> {
        let (_, family) = resolve_family(font_index)?;
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

/* ---------------------------------------------------------------------------
 *  内置七段数码（电子表 / LCD 风格）
 *
 *  Windows 没有自带七段码字体，引用外部字体名（DS-Digital 之类）机器上没装就
 *  落空，所以干脆自己画：每段是一个六边形（两端削尖，经典 LCD 的样子），
 *  用 GdipFillPolygon 填 —— 和文字走同一块画布、同一条「画一次扫墨迹」的
 *  排版管线，抗锯齿和描边（八方向重画）也是同一套。
 *
 *  字号口径和文字模式对齐：数码高度 ≈ font_px × 0.66，接近 Consolas 数字的
 *  墨迹高度（约 0.63 em），同一档字号下两种模式看起来一般大。
 * ------------------------------------------------------------------------ */

/// 每个数字点亮哪几段：bit0=a（上横）… bit6=g（中横），经典七段编码。
const SEGMENT_TABLE: [u8; 10] = [
    0b0111111, // 0: a b c d e f
    0b0000110, // 1: b c
    0b1011011, // 2: a b g e d
    0b1001111, // 3: a b c d g
    0b1100110, // 4: f g b c
    0b1101101, // 5: a f g c d
    0b1111101, // 6: a f g e c d
    0b0000111, // 7: a b c
    0b1111111, // 8: 全亮
    0b1101111, // 9: a b c d f g
];

const SEG_A: u8 = 1 << 0;
const SEG_B: u8 = 1 << 1;
const SEG_C: u8 = 1 << 2;
const SEG_D: u8 = 1 << 3;
const SEG_E: u8 = 1 << 4;
const SEG_F: u8 = 1 << 5;
const SEG_G: u8 = 1 << 6;

/// 一段的水平/垂直六边形。横段给 (x, y, 长)，竖段同函数转置着用。
unsafe fn fill_segment_h(
    graphics: *mut GpGraphics,
    brush: *mut GpBrush,
    x: f32,
    y: f32,
    len: f32,
    t: f32,
) {
    let half = t / 2.0;
    let points = [
        PointF { X: x + half, Y: y },
        PointF { X: x + len - half, Y: y },
        PointF { X: x + len, Y: y + half },
        PointF { X: x + len - half, Y: y + t },
        PointF { X: x + half, Y: y + t },
        PointF { X: x, Y: y + half },
    ];
    let _ = GdipFillPolygon(graphics, brush, points.as_ptr(), 6, FillModeAlternate);
}

unsafe fn fill_segment_v(
    graphics: *mut GpGraphics,
    brush: *mut GpBrush,
    x: f32,
    y: f32,
    len: f32,
    t: f32,
) {
    let half = t / 2.0;
    let points = [
        PointF { X: x + half, Y: y },
        PointF { X: x + t, Y: y + half },
        PointF { X: x + t, Y: y + len - half },
        PointF { X: x + half, Y: y + len },
        PointF { X: x, Y: y + len - half },
        PointF { X: x, Y: y + half },
    ];
    let _ = GdipFillPolygon(graphics, brush, points.as_ptr(), 6, FillModeAlternate);
}

unsafe fn draw_segment_digit(
    graphics: *mut GpGraphics,
    brush: *mut GpBrush,
    digit: usize,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    t: f32,
) {
    let on = SEGMENT_TABLE[digit];
    // 横段两端留出削尖的空隙，竖段插进横段之间的缝里
    let hx = x + t * 0.6;
    let hl = w - t * 1.2;
    let vy1 = y + t * 0.6;
    let vl = h / 2.0 - t * 1.1;
    let vy2 = y + h / 2.0 + t * 0.5;

    if on & SEG_A != 0 {
        fill_segment_h(graphics, brush, hx, y, hl, t);
    }
    if on & SEG_G != 0 {
        fill_segment_h(graphics, brush, hx, y + (h - t) / 2.0, hl, t);
    }
    if on & SEG_D != 0 {
        fill_segment_h(graphics, brush, hx, y + h - t, hl, t);
    }
    if on & SEG_F != 0 {
        fill_segment_v(graphics, brush, x, vy1, vl, t);
    }
    if on & SEG_B != 0 {
        fill_segment_v(graphics, brush, x + w - t, vy1, vl, t);
    }
    if on & SEG_E != 0 {
        fill_segment_v(graphics, brush, x, vy2, vl, t);
    }
    if on & SEG_C != 0 {
        fill_segment_v(graphics, brush, x + w - t, vy2, vl, t);
    }
}

/// 一格的步进宽度（数字 / 冒号共用几何参数，好让总宽度可算）。
struct SegmentMetrics {
    digit_w: f32,
    colon_w: f32,
    gap: f32,
    h: f32,
    t: f32,
}

fn segment_metrics(font_px: f32) -> SegmentMetrics {
    let h = font_px * 0.66;
    let t = h / 7.0;
    SegmentMetrics {
        digit_w: h * 0.52,
        colon_w: t * 2.0,
        gap: t * 1.2,
        h,
        t,
    }
}

/// 整串（固定 "HH:MM:SS" 八字符）的像素宽度。
fn segment_string_width(text: &[u16], m: &SegmentMetrics) -> f32 {
    let mut width = 0.0f32;
    let mut cells = 0;
    for &c in text.iter().take_while(|&&c| c != 0) {
        width += if c == b':' as u16 { m.colon_w } else { m.digit_w };
        cells += 1;
    }
    if cells > 1 {
        width += m.gap * (cells - 1) as f32;
    }
    width
}

/// 在 (x, y) 处把整串时间画出来（x 是串的左缘，y 是数码顶缘）。
unsafe fn draw_segment_string(
    graphics: *mut GpGraphics,
    brush: *mut GpBrush,
    text: &[u16],
    x: f32,
    y: f32,
    m: &SegmentMetrics,
) {
    let mut cursor = x;
    for &c in text.iter().take_while(|&&c| c != 0) {
        if c == b':' as u16 {
            // 冒号：上下两个方点，垂直方向摆在 1/3 和 2/3 处
            let dots = [
                PointF { X: cursor, Y: y + m.h * 0.28 },
                PointF { X: cursor + m.t, Y: y + m.h * 0.28 },
                PointF { X: cursor + m.t, Y: y + m.h * 0.28 + m.t },
                PointF { X: cursor, Y: y + m.h * 0.28 + m.t },
            ];
            let _ = GdipFillPolygon(graphics, brush, dots.as_ptr(), 4, FillModeAlternate);
            let dots = [
                PointF { X: cursor, Y: y + m.h * 0.72 - m.t },
                PointF { X: cursor + m.t, Y: y + m.h * 0.72 - m.t },
                PointF { X: cursor + m.t, Y: y + m.h * 0.72 },
                PointF { X: cursor, Y: y + m.h * 0.72 },
            ];
            let _ = GdipFillPolygon(graphics, brush, dots.as_ptr(), 4, FillModeAlternate);
            cursor += m.colon_w + m.gap;
        } else if (b'0' as u16..=b'9' as u16).contains(&c) {
            draw_segment_digit(
                graphics,
                brush,
                (c - b'0' as u16) as usize,
                cursor,
                y,
                m.digit_w,
                m.h,
                m.t,
            );
            cursor += m.digit_w + m.gap;
        } else {
            cursor += m.digit_w + m.gap; // 不认识的字符留一空位，别把串挤歪
        }
    }
}

/// 七段数码版的 render_text：同样的排版矩形约定（TEXT_RECT_SIZE 见方、
/// 串在矩形里居中），同样的八方向描边套路，于是排版管线（画一次扫墨迹、
/// 矩形平移量 = 墨迹平移量）原样适用。
pub fn render_segments(
    graphics: *mut GpGraphics,
    text: &[u16],
    rect_x: f32,
    rect_y: f32,
    font_px: f32,
    color: u32,
    outline_color: u32,
    outline: i32,
) {
    if graphics.is_null() || text.is_empty() || text[0] == 0 {
        return;
    }

    let m = segment_metrics(font_px);
    let width = segment_string_width(text, &m);
    // 矩形尺寸固定，居中放串：起点 = 矩形左上角 + (矩形尺寸 - 串尺寸) / 2
    let x = rect_x + (TEXT_RECT_SIZE - width) / 2.0;
    let y = rect_y + (TEXT_RECT_SIZE - m.h) / 2.0;

    unsafe {
        if outline > 0 {
            let mut edge: *mut GpSolidFill = ptr::null_mut();
            if gp_ok(GdipCreateSolidFill(outline_color, &mut edge)) {
                let mut dy = -outline;
                while dy <= outline {
                    let mut dx = -outline;
                    while dx <= outline {
                        if !(dx == 0 && dy == 0) {
                            draw_segment_string(
                                graphics,
                                edge.cast::<GpBrush>(),
                                text,
                                x + dx as f32,
                                y + dy as f32,
                                &m,
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
            draw_segment_string(graphics, fill.cast::<GpBrush>(), text, x, y, &m);
            let _ = GdipDeleteBrush(fill.cast::<GpBrush>());
        }
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
