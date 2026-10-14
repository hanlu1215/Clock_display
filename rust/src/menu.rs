//! 右键菜单：颜色（含自定义）/ 字号 / 描边 / 置顶 / 关闭（对应 C 版 menu.c）

use std::cell::UnsafeCell;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{COLORREF, POINT};
use windows::Win32::UI::Controls::Dialogs::{ChooseColorW, CHOOSECOLORW, CC_FULLOPEN, CC_RGBINIT};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, PostMessageW, SetForegroundWindow,
    TrackPopupMenu, HMENU, MF_CHECKED, MF_POPUP, MF_SEPARATOR, MF_STRING, MF_UNCHECKED,
    TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON, WM_NULL,
};

use crate::clock::Clock;
use crate::config::{COLOR_CHOICES, SIZE_STEPS};
use crate::util::to_wide;

// 菜单项 id：颜色和字号各占一段连续区间，按下标反查选项。
const MENU_COLOR_BASE: usize = 2000;
const MENU_SIZE_BASE: usize = 2100;
const MENU_COLOR_CUSTOM: usize = 2200;
const MENU_OUTLINE: usize = 2201;
const MENU_TOPMOST: usize = 2202;
const MENU_QUIT: usize = 2203;

// ChooseColorW 的「自定义颜色」格子：得跨次调用留着，所以放静态。
// 不放的话每次打开颜色对话框，上次调好的颜色就没了。
// 菜单是模态的、只在 GUI 线程上跑，不会有并发访问。
struct SyncCell<T>(UnsafeCell<T>);
unsafe impl<T> Sync for SyncCell<T> {}
static CUSTOM_COLORS: SyncCell<[u32; 16]> = SyncCell(UnsafeCell::new([0; 16]));

fn build_color_menu(clock: &Clock) -> Option<HMENU> {
    let current = clock.settings.color;
    unsafe {
        let menu = CreatePopupMenu().ok()?;
        let mut matched = false;

        for (i, choice) in COLOR_CHOICES.iter().enumerate() {
            let mut flags = MF_STRING;
            if choice.argb == current {
                flags |= MF_CHECKED;
                matched = true;
            }
            let name = to_wide(choice.name);
            let _ = AppendMenuW(menu, flags, MENU_COLOR_BASE + i, PCWSTR(name.as_ptr()));
        }

        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        // 当前颜色不在预设里，就说明是自己调的，把勾打在这一项上
        let label = to_wide("自定义…");
        let _ = AppendMenuW(
            menu,
            MF_STRING | if matched { MF_UNCHECKED } else { MF_CHECKED },
            MENU_COLOR_CUSTOM,
            PCWSTR(label.as_ptr()),
        );
        Some(menu)
    }
}

fn build_size_menu(clock: &Clock) -> Option<HMENU> {
    let current = clock.settings.size_index;
    unsafe {
        let menu = CreatePopupMenu().ok()?;

        for (i, &size) in SIZE_STEPS.iter().enumerate() {
            let label = to_wide(&format!("{} 像素", size));
            let _ = AppendMenuW(
                menu,
                MF_STRING | if i == current { MF_CHECKED } else { MF_UNCHECKED },
                MENU_SIZE_BASE + i,
                PCWSTR(label.as_ptr()),
            );
        }
        Some(menu)
    }
}

fn choose_custom_color(clock: &mut Clock) {
    let current = clock.settings.color;
    unsafe {
        let mut dialog = CHOOSECOLORW {
            lStructSize: std::mem::size_of::<CHOOSECOLORW>() as u32,
            hwndOwner: clock.hwnd,
            rgbResult: COLORREF(
                ((current >> 16) & 0xFF) | (((current >> 8) & 0xFF) << 8) | ((current & 0xFF) << 16),
            ),
            lpCustColors: CUSTOM_COLORS.0.get() as *mut COLORREF,
            Flags: CC_FULLOPEN | CC_RGBINIT,
            ..Default::default()
        };

        if !ChooseColorW(&mut dialog).as_bool() {
            return; // 点了取消
        }

        // COLORREF 是 0x00BBGGRR，转成程序内部用的 0xAARRGGBB（并补上不透明的 alpha）
        let rgb = dialog.rgbResult.0;
        clock.set_color(
            0xFF000000 | ((rgb & 0xFF) << 16) | (rgb & 0xFF00) | ((rgb >> 16) & 0xFF),
        );
    }
}

fn handle_command(clock: &mut Clock, command: usize) {
    if (MENU_COLOR_BASE..MENU_COLOR_BASE + COLOR_CHOICES.len()).contains(&command) {
        clock.set_color(COLOR_CHOICES[command - MENU_COLOR_BASE].argb);
        return;
    }
    if (MENU_SIZE_BASE..MENU_SIZE_BASE + SIZE_STEPS.len()).contains(&command) {
        clock.set_size_index((command - MENU_SIZE_BASE) as i32);
        return;
    }

    match command {
        MENU_COLOR_CUSTOM => choose_custom_color(clock),
        MENU_OUTLINE => clock.set_outline(!clock.settings.outline),
        MENU_TOPMOST => clock.set_topmost(!clock.settings.topmost),
        MENU_QUIT => clock.quit(),
        _ => {} // 点在菜单外面关掉了：什么都不做
    }
}

pub fn show(clock: &mut Clock) {
    let window = clock.hwnd;
    unsafe {
        let root = CreatePopupMenu();
        let colors = build_color_menu(clock);
        let sizes = build_size_menu(clock);

        let (Ok(root), Some(colors), Some(sizes)) = (root, colors, sizes) else {
            return; // 建不出来就算了：这次右键当作没点
        };

        let label_color = to_wide("颜色");
        let label_size = to_wide("字号");
        let label_outline = to_wide("描边");
        let label_topmost = to_wide("置顶");
        let label_quit = to_wide("关闭");

        let _ = AppendMenuW(root, MF_POPUP, colors.0 as usize, PCWSTR(label_color.as_ptr()));
        let _ = AppendMenuW(root, MF_POPUP, sizes.0 as usize, PCWSTR(label_size.as_ptr()));
        let _ = AppendMenuW(root, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(
            root,
            MF_STRING | if clock.settings.outline { MF_CHECKED } else { MF_UNCHECKED },
            MENU_OUTLINE,
            PCWSTR(label_outline.as_ptr()),
        );
        let _ = AppendMenuW(
            root,
            MF_STRING | if clock.settings.topmost { MF_CHECKED } else { MF_UNCHECKED },
            MENU_TOPMOST,
            PCWSTR(label_topmost.as_ptr()),
        );
        let _ = AppendMenuW(root, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(root, MF_STRING, MENU_QUIT, PCWSTR(label_quit.as_ptr()));

        let mut cursor = POINT::default();
        let _ = GetCursorPos(&mut cursor);

        // TrackPopupMenu 要求窗口在前台，否则菜单弹出来后点别处不会消失
        let _ = SetForegroundWindow(window);
        let command = TrackPopupMenu(
            root,
            TPM_RIGHTBUTTON | TPM_RETURNCMD | TPM_NONOTIFY,
            cursor.x,
            cursor.y,
            Some(0),
            window,
            None,
        );
        let _ = DestroyMenu(root); // 递归的，子菜单一起销毁

        // 喂掉菜单留下的空消息，不然前台锁会卡住下一次菜单
        let _ = PostMessageW(Some(window), WM_NULL, Default::default(), Default::default());

        handle_command(clock, command.0 as usize);
    }
}
