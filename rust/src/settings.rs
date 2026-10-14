//! clock.ini 读写（对应 C 版 settings.c）
//!
//! 存档路径 = exe 所在目录 + clock.ini。
//! 之所以跟着 exe 而不是放 %APPDATA%：绿色软件，整个文件夹拷到别的机器上，
//! 位置、颜色、字号一起跟着走。
//! 节名、键名、值的格式都和 C 版一致，两版的 clock.ini 可以直接互换。

use windows::core::PCWSTR;
use windows::Win32::Storage::FileSystem::DeleteFileW;
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;
use windows::Win32::System::WindowsProgramming::{
    GetPrivateProfileStringW, WritePrivateProfileStringW,
};

use crate::config::{
    DEFAULT_COLOR, DEFAULT_OUTLINE, DEFAULT_SIZE_INDEX, DEFAULT_TOPMOST, INI_NAME, INI_SECTION,
    POS_UNSET, SIZE_STEPS,
};
use crate::util::{clamp_int, to_wide};

pub struct Settings {
    pub color: u32, // 0xAARRGGBB
    pub size_index: usize,
    pub x: i32,
    pub y: i32,
    pub topmost: bool,
    pub outline: bool,
}

impl Settings {
    pub fn defaults() -> Settings {
        Settings {
            color: DEFAULT_COLOR,
            size_index: DEFAULT_SIZE_INDEX,
            x: POS_UNSET,
            y: POS_UNSET,
            topmost: DEFAULT_TOPMOST,
            outline: DEFAULT_OUTLINE,
        }
    }
}

/// exe 所在目录 + clock.ini；连自己在哪儿都不知道就返回 None（调用方用默认值）。
fn settings_path() -> Option<Vec<u16>> {
    unsafe {
        let mut buffer = vec![0u16; 260]; // MAX_PATH
        let length = GetModuleFileNameW(None, &mut buffer);
        if length == 0 || length as usize >= buffer.len() {
            return None;
        }
        buffer.truncate(length as usize);

        // 砍掉文件名，只留目录
        let slash = buffer.iter().rposition(|&c| c == b'\\' as u16)?;
        buffer.truncate(slash + 1);

        let mut path = buffer;
        path.extend(to_wide(INI_NAME));
        Some(path)
    }
}

fn ini_read(path: &[u16], key: &str) -> Option<String> {
    let section = to_wide(INI_SECTION);
    let key = to_wide(key);
    let empty = to_wide("");
    let mut text = vec![0u16; 64];
    unsafe {
        let length = GetPrivateProfileStringW(
            PCWSTR(section.as_ptr()),
            PCWSTR(key.as_ptr()),
            PCWSTR(empty.as_ptr()),
            Some(&mut text),
            PCWSTR(path.as_ptr()),
        );
        if length == 0 {
            return None;
        }
        text.truncate(length as usize);
        Some(String::from_utf16_lossy(&text))
    }
}

/// 用字符串再 parse 而不是 GetPrivateProfileIntW：副屏摆在主屏左边时坐标是负数，
/// 那个接口只认无符号数。
fn ini_int(path: &[u16], key: &str, fallback: i32) -> i32 {
    match ini_read(path, key) {
        Some(text) => text.trim().parse().unwrap_or(fallback),
        None => fallback,
    }
}

fn ini_hex(path: &[u16], key: &str, fallback: u32) -> u32 {
    match ini_read(path, key) {
        Some(text) => u32::from_str_radix(text.trim(), 16).unwrap_or(fallback),
        None => fallback,
    }
}

fn ini_write(path: &[u16], key: &str, value: &str) {
    let section = to_wide(INI_SECTION);
    let key = to_wide(key);
    let value = to_wide(value);
    unsafe {
        let _ = WritePrivateProfileStringW(
            PCWSTR(section.as_ptr()),
            PCWSTR(key.as_ptr()),
            PCWSTR(value.as_ptr()),
            PCWSTR(path.as_ptr()),
        );
    }
}

pub fn load() -> Settings {
    let mut settings = Settings::defaults();

    let Some(path) = settings_path() else {
        return settings;
    };

    settings.size_index = clamp_int(
        ini_int(&path, "size", DEFAULT_SIZE_INDEX as i32),
        0,
        SIZE_STEPS.len() as i32 - 1,
    ) as usize;
    settings.x = ini_int(&path, "x", POS_UNSET);
    settings.y = ini_int(&path, "y", POS_UNSET);
    settings.topmost = ini_int(&path, "topmost", DEFAULT_TOPMOST as i32) != 0;
    settings.outline = ini_int(&path, "outline", DEFAULT_OUTLINE as i32) != 0;

    let mut color = ini_hex(&path, "color", DEFAULT_COLOR);
    if (color >> 24) == 0 {
        color |= 0xFF000000; // 存档里没带 alpha 就补成不透明
    }
    settings.color = color;

    settings
}

pub fn save(settings: &Settings) {
    let Some(path) = settings_path() else {
        return;
    };

    // 先把整节删掉再写：存档里不会留下上一版程序的旧键。
    let section = to_wide(INI_SECTION);
    unsafe {
        let _ = WritePrivateProfileStringW(
            PCWSTR(section.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            PCWSTR(path.as_ptr()),
        );
    }

    ini_write(&path, "color", &format!("{:08X}", settings.color));
    ini_write(&path, "size", &settings.size_index.to_string());
    ini_write(&path, "x", &settings.x.to_string());
    ini_write(&path, "y", &settings.y.to_string());
    ini_write(&path, "topmost", if settings.topmost { "1" } else { "0" });
    ini_write(&path, "outline", if settings.outline { "1" } else { "0" });
}

pub fn reset() {
    if let Some(path) = settings_path() {
        unsafe {
            // 删不掉（没有写权限之类）也无所谓，下次照样按存档跑
            let _ = DeleteFileW(PCWSTR(path.as_ptr()));
        }
    }
}
