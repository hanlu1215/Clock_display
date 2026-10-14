/* ===========================================================================
 *  桌面时钟 —— Rust 版（对应根目录的 C 版，功能 1:1）
 *
 *  屏幕上只有一行时间，背景全透明：
 *      左键拖动  -> 移动，只有文字本身点得中，透明处鼠标会穿过去
 *      右键      -> 菜单：颜色 / 字号 / 描边 / 置顶 / 关闭
 *      鼠标滚轮  -> 调字号
 *      Esc       -> 关闭
 *
 *  文字样式参考 win_agent-notify-C 隐私屏幕里那只钟（Consolas 粗体、纯白、
 *  背后一层黑描边、24 小时制 %H:%M:%S），区别只在于背景透明而不是黑底。
 *
 *  设置存在 exe 同目录的 clock.ini 里（格式和 C 版一致，可互换）。
 * ======================================================================== */

// 窗口子系统：双击启动不会先弹一个命令提示符窗口，正是要的效果（同 C 版 WIN32 目标）
#![windows_subsystem = "windows"]

mod clock;
mod config;
mod menu;
mod render;
mod settings;
mod surface;
mod util;

use std::ptr;

use windows::core::{BOOL, PCWSTR};
use windows::Win32::Graphics::GdiPlus::{GdiplusShutdown, GdiplusStartup, GdiplusStartupInput};
use windows::Win32::UI::WindowsAndMessaging::{
    MessageBoxW, SetProcessDPIAware, MB_ICONERROR, MB_ICONINFORMATION, MB_OK,
};

use clock::Clock;
use util::to_wide;

const APP_TITLE: &str = "桌面时钟";

struct Options {
    reset: bool, // --reset：把存档删掉，回到默认位置/字号/颜色
    help: bool,
}

fn parse_command_line() -> Options {
    let mut options = Options {
        reset: false,
        help: false,
    };

    // std::env::args_os 走的就是 CRT 对 GetCommandLineW 的那套解析，
    // 和 C 版手动 CommandLineToArgvW 等价
    for arg in std::env::args_os().skip(1) {
        if arg == "--reset" || arg == "-r" {
            options.reset = true;
        } else if arg == "--help" || arg == "-h" || arg == "/?" {
            options.help = true;
        }
    }
    options
}

/// 打包成窗口程序（没有控制台）之后，println 是没人看的，要跟用户说话只能弹框。
/// 帮助就是这么给的。
fn show_help() {
    let text = to_wide(
        "桌面时钟 —— 屏幕上只剩一行时间，背景全透明\n\
         \n\
         用法：clock_display.exe [选项]\n\
         \n\
         \x20 -r, --reset    恢复默认位置、字号和颜色\n\
         \x20                （时钟被拖到屏幕外找不回来时用这个）\n\
         \x20 -h, --help     显示这份帮助\n\
         \n\
         操作\n\
         \x20 左键拖动    移动时钟（只有文字本身点得中，透明处鼠标会穿过去）\n\
         \x20 鼠标滚轮    调字号\n\
         \x20 右键        菜单：颜色 / 字号 / 描边 / 置顶 / 关闭\n\
         \x20 Esc         关闭\n\
         \n\
         设置自动存在 exe 同目录的 clock.ini 里。",
    );
    let title = to_wide(APP_TITLE);
    unsafe {
        MessageBoxW(
            None,
            PCWSTR(text.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK | MB_ICONINFORMATION,
        );
    }
}

fn show_error(text: &str) {
    let text = to_wide(text);
    let title = to_wide(APP_TITLE);
    unsafe {
        MessageBoxW(
            None,
            PCWSTR(text.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK | MB_ICONERROR,
        );
    }
}

fn main() {
    let options = parse_command_line();
    if options.help {
        show_help();
        return;
    }

    unsafe {
        // 清单里已经声明了 dpiAware，这里再兜一次底：万一清单没生效，
        // 高 DPI 下也不至于被系统拉成一团糊。
        let _ = SetProcessDPIAware();

        let startup_input = GdiplusStartupInput {
            GdiplusVersion: 1,
            DebugEventCallback: 0,
            SuppressBackgroundThread: BOOL(0),
            SuppressExternalCodecs: BOOL(0),
        };
        let mut gdiplus_token: usize = 0;
        if GdiplusStartup(&mut gdiplus_token, &startup_input, ptr::null_mut()).0 != 0 {
            show_error("GDI+ 初始化失败，画不了字，程序退出。");
            std::process::exit(2);
        }

        let exit_code = {
            // 这个作用域很关键：所有 GDI+ 对象都得在 GdiplusShutdown 之前析构干净。
            if render::family().is_none() {
                show_error(
                    "系统里找不到任何能用的字体（Consolas、微软雅黑、Segoe UI 都没有），没法画时间。",
                );
                2
            } else {
                match Clock::create(options.reset) {
                    None => {
                        show_error("窗口创建失败。");
                        2
                    }
                    Some(clock) => {
                        clock.run();
                        clock.destroy();
                        0
                    }
                }
            }
        };

        GdiplusShutdown(gdiplus_token);
        std::process::exit(exit_code);
    }
}
