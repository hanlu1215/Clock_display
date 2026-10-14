//! 小工具：DPI、取时间、亮度判断（对应 C 版 util.c）

use windows::Win32::Graphics::Gdi::{GetDC, GetDeviceCaps, ReleaseDC, LOGPIXELSX};
use windows::Win32::System::SystemInformation::GetLocalTime;

/// &str → 以 0 结尾的 UTF-16，Win32 W 系 API 都要这个。
pub fn to_wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn dpi_scale() -> f64 {
    unsafe {
        let screen = GetDC(None);
        let mut dpi = 96;
        if !screen.is_invalid() {
            let value = GetDeviceCaps(Some(screen), LOGPIXELSX);
            ReleaseDC(None, screen);
            if value > 0 {
                dpi = value;
            }
        }
        dpi as f64 / 96.0
    }
}

/// "HH:MM:SS"，24 小时制，和隐私屏幕那只钟一致。
pub fn format_time() -> String {
    let now = unsafe { GetLocalTime() };
    format!(
        "{:02}:{:02}:{:02}",
        now.wHour, now.wMinute, now.wSecond
    )
}

/// 人眼对绿最敏感，红蓝次之 —— 加权亮度过半就算亮色。
pub fn color_is_light(argb: u32) -> bool {
    let r = ((argb >> 16) & 0xFF) as i32;
    let g = ((argb >> 8) & 0xFF) as i32;
    let b = (argb & 0xFF) as i32;
    (r * 299 + g * 587 + b * 114) / 1000 >= 128
}

pub fn clamp_int(value: i32, low: i32, high: i32) -> i32 {
    value.clamp(low, high)
}
