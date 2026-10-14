//! ===========================================================================
//!  时钟本体：一个无边框、背景全透明的分层窗口，上面只画一行时间
//!  （对应 C 版 clock.c）。
//!
//!  分层窗口在 alpha 为 0 的地方本来就把鼠标消息放行给下面的窗口，
//!  于是「只有文字能点中」是白送的 —— 不用自己去算命中区域，
//!  也就正好是想要的交互：点着文字能拖，透明处戳不着。
//! ========================================================================

use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromPoint, HMONITOR, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::Graphics::GdiPlus::{
    GdipSetPixelOffsetMode, GdipSetSmoothingMode, GdipSetTextRenderingHint, PixelOffsetModeHalf,
    SmoothingModeAntiAlias, TextRenderingHintAntiAlias,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetCapture, ReleaseCapture, SetCapture, VK_ESCAPE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetCursorPos,
    GetMessageW, GetSystemMetrics, GetWindowLongPtrW, GetWindowRect, KillTimer, LoadCursorW,
    PostQuitMessage, RegisterClassExW, SetForegroundWindow, SetTimer, SetWindowLongPtrW,
    SetWindowPos, ShowWindow, SystemParametersInfoW, TranslateMessage, UnregisterClassW,
    CREATESTRUCTW, GWLP_USERDATA, HWND_NOTOPMOST, HWND_TOPMOST, IDC_SIZEALL,
    MSG, SM_CXSCREEN, SM_CYSCREEN, SPI_GETWORKAREA, SWP_NOACTIVATE,
    SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SW_SHOWNOACTIVATE, WINDOW_EX_STYLE, WNDCLASSEXW,
    WS_POPUP, WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WM_CAPTURECHANGED, WM_CLOSE,
    WM_DESTROY, WM_DISPLAYCHANGE, WM_ERASEBKGND, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP,
    WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_NCCREATE, WM_RBUTTONUP, WM_TIMER,
};

use crate::config::{
    DRAG_THRESHOLD, OUTLINE_DIVISOR, PAD_MIN, POS_UNSET, SIZE_STEPS, START_TOP_RATIO, TICK_MS,
};
use crate::menu;
use crate::render::{self, Font, TEXT_RECT_SIZE};
use crate::settings::{self, Settings};
use crate::surface::Surface;
use crate::util;

const TIMER_TICK: usize = 1;

const WINDOW_CLASS: &str = "ClockDisplayWindow";
const WINDOW_TITLE: &str = "桌面时钟";

/// 量排版时用的替身：全 8 的等宽字符串。
/// Consolas 这类等宽字体里，每个数字和冒号的步进宽度完全一样，所以「88:88:88」
/// 占的地方和任意时刻的真实时间分毫不差 —— 拿它定窗口大小，秒数再怎么跳，
/// 窗口也纹丝不动。
const PROBE_TEXT: &str = "88:88:88";

pub struct Clock {
    pub hwnd: HWND,
    surface: Surface,
    font: Option<Font>,
    pub settings: Settings,

    dpi_scale: f64,
    font_px: f32,      // 这一档字号实际用到的像素大小
    outline: i32,      // 描边半径（像素）
    win_w: i32,
    win_h: i32,        // 窗口 / 画布尺寸
    rect_x: f32,
    rect_y: f32,       // DrawString 布局矩形的左上角

    time_text: String,
    topmost_stamp: String, // 上次重申置顶时的 "HH:MM"，分没跳就不重申

    pressed: bool,
    dragging: bool,
    press_cursor: POINT,
    press_origin: POINT,
}

// ---------------------------------------------------------------- 位置

fn work_area() -> RECT {
    unsafe {
        let mut work = RECT::default();
        if SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some(&mut work as *mut RECT as *mut std::ffi::c_void),
            Default::default(),
        )
        .is_err()
        {
            work.left = 0;
            work.top = 0;
            work.right = GetSystemMetrics(SM_CXSCREEN);
            work.bottom = GetSystemMetrics(SM_CYSCREEN);
        }
        work
    }
}

/// 把窗口按回它所在那台显示器的可用区域里。
/// 换过显示器、改过分辨率之后，存档里的坐标可能已经落到屏幕外，
/// 不夹一下的话时钟就「消失」了 —— 明明在跑，就是看不见。
fn clamp_position(x: &mut i32, y: &mut i32, w: i32, h: i32) {
    unsafe {
        let center = POINT {
            x: *x + w / 2,
            y: *y + h / 2,
        };
        let monitor: HMONITOR = MonitorFromPoint(center, MONITOR_DEFAULTTONEAREST);
        if monitor.is_invalid() {
            return;
        }
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(monitor, &mut info).as_bool() {
            return;
        }
        if *x + w > info.rcWork.right {
            *x = info.rcWork.right - w;
        }
        if *y + h > info.rcWork.bottom {
            *y = info.rcWork.bottom - h;
        }
        if *x < info.rcWork.left {
            *x = info.rcWork.left;
        }
        if *y < info.rcWork.top {
            *y = info.rcWork.top;
        }
    }
}

impl Clock {
    fn remember_position(&mut self) {
        unsafe {
            if self.hwnd.is_invalid() {
                return;
            }
            let mut rect = RECT::default();
            if GetWindowRect(self.hwnd, &mut rect).is_ok() {
                self.settings.x = rect.left;
                self.settings.y = rect.top;
                settings::save(&self.settings);
            }
        }
    }

    /// 亮字配黑描边、暗字配白描边，压在任何壁纸上都读得清。
    fn outline_color(&self) -> u32 {
        if util::color_is_light(self.settings.color) {
            0xFF000000
        } else {
            0xFFFFFFFF
        }
    }

    // ---------------------------------------------------------------- 排版

    /// 画一遍、扫一遍，量出窗口该多大、文字该摆在哪儿。
    ///
    /// 为什么不用 GdipMeasureString 的结果当窗口尺寸：它量出来的是「行框」，
    /// 上伸部和下伸部都留着空档 —— 数字既够不到顶、也踩不到底，照着裁出来的窗口
    /// 上面紧、下面松，一眼就看出是歪的；而且它量出来的框还比字本身大一圈。
    /// 所以这里改成真画一次，再扫 alpha 找出墨迹的真实范围（描边也算在内），拿它反推。
    fn measure_layout(&mut self) -> Option<(i32, i32, f32, f32)> {
        let font = self.font.as_ref()?;

        // 墨迹四周留白：描边半径 + 一点呼吸空间，别让描边贴着窗口边被切掉
        let mut pad = self.outline + (self.font_px / 32.0) as i32;
        if pad < self.outline + PAD_MIN {
            pad = self.outline + PAD_MIN;
        }

        // 临时画布开得足够放肆：8 个字符最宽也占不到 5 个字宽，这边给了 10 个
        // （高度 3 个），再多的富余也只是临时位图，不花什么代价，却能保证
        // 「88:88:88」一次画全、不被画布边缘切掉。
        let probe_w = self.font_px as i32 * 10 + 64;
        let probe_h = self.font_px as i32 * 3 + 64;
        if !self.surface.ensure(probe_w, probe_h) {
            return None;
        }

        // 排版矩形固定 TEXT_RECT_SIZE 见方，让它的中心落在临时画布正中 ——
        // 这样文字就居中画在画布上，扫出来的墨迹稳稳落在框内。
        let origin_x = probe_w as f32 / 2.0 - TEXT_RECT_SIZE / 2.0;
        let origin_y = probe_h as f32 / 2.0 - TEXT_RECT_SIZE / 2.0;

        self.surface.clear();
        render::render_text(
            self.surface.graphics,
            &util::to_wide(PROBE_TEXT),
            font,
            origin_x,
            origin_y,
            self.settings.color,
            self.outline_color(),
            self.outline,
        );
        self.surface.flush();
        let ink = render::ink_bounds(self.surface.bits, probe_w, probe_h)?;

        let win_w = (ink.right - ink.left + 1) + pad * 2;
        let win_h = (ink.bottom - ink.top + 1) + pad * 2;

        // 矩形尺寸是定死的，所以「矩形左上角挪多少，墨迹就跟着挪多少」是个线性关系：
        // 想让墨迹落在 (pad, pad)，把这次的左上角平移同样的差值即可。
        let rect_x = origin_x + (pad - ink.left) as f32;
        let rect_y = origin_y + (pad - ink.top) as f32;
        Some((win_w, win_h, rect_x, rect_y))
    }

    /// 按当前字号重排，并把窗口摆好。
    /// keep_center = true 时以文字中心为锚点（改字号别让时钟蹦到别处去）。
    fn relayout(&mut self, keep_center: bool) -> bool {
        self.font_px = SIZE_STEPS[self.settings.size_index] as f32 * self.dpi_scale as f32;
        // 描边半径按字号等比缩放；关掉的时候就是 0 —— 少了这一步的话，
        // 菜单里的勾选、存档、读档全都好好的，只有画面永远带着描边。
        if self.settings.outline {
            self.outline = (self.font_px / OUTLINE_DIVISOR + 0.5) as i32;
            if self.outline < 1 {
                self.outline = 1;
            }
        } else {
            self.outline = 0;
        }

        self.font = Font::create(self.font_px);
        if self.font.is_none() {
            return false;
        }

        let Some((width, height, rect_x, rect_y)) = self.measure_layout() else {
            return false;
        };

        self.win_w = width;
        self.win_h = height;
        self.rect_x = rect_x;
        self.rect_y = rect_y;

        let mut x;
        let mut y;
        unsafe {
            if keep_center && !self.hwnd.is_invalid() {
                let mut rect = RECT::default();
                let _ = GetWindowRect(self.hwnd, &mut rect);
                x = rect.left + (rect.right - rect.left) / 2 - width / 2;
                y = rect.top + (rect.bottom - rect.top) / 2 - height / 2;
            } else {
                x = self.settings.x;
                y = self.settings.y;
                if x == POS_UNSET || y == POS_UNSET {
                    // 头一回运行：水平居中，纵向离工作区顶部一点距离
                    let work = work_area();
                    x = work.left + (work.right - work.left - width) / 2;
                    y = work.top + ((work.bottom - work.top) as f64 * START_TOP_RATIO) as i32;
                }
            }

            clamp_position(&mut x, &mut y, width, height);
            self.settings.x = x;
            self.settings.y = y;

            let _ = SetWindowPos(
                self.hwnd,
                Some(if self.settings.topmost {
                    HWND_TOPMOST
                } else {
                    HWND_NOTOPMOST
                }),
                x,
                y,
                width,
                height,
                SWP_NOACTIVATE,
            );
        }
        true
    }

    // ---------------------------------------------------------------- 绘制

    fn render(&mut self) {
        if self.hwnd.is_invalid() || self.font.is_none() {
            return;
        }
        if !self.surface.ensure(self.win_w, self.win_h) {
            return;
        }
        let graphics = self.surface.graphics;
        if graphics.is_null() {
            return;
        }

        self.surface.clear();
        unsafe {
            let _ = GdipSetSmoothingMode(graphics, SmoothingModeAntiAlias);
            let _ = GdipSetPixelOffsetMode(graphics, PixelOffsetModeHalf);
            // 分层窗口开不了 ClearType：次像素抗锯齿得靠不透明的底色，而这儿的底色
            // 是全透明的。灰阶抗锯齿（AntiAlias）配 alpha 通道才是正解 ——
            // 隐私屏幕那只钟用的是 CLEARTYPE_QUALITY，但它是画在不透明黑底上的。
            let _ = GdipSetTextRenderingHint(graphics, TextRenderingHintAntiAlias);
        }

        render::render_text(
            graphics,
            &util::to_wide(&self.time_text.clone()),
            self.font.as_ref().unwrap(),
            self.rect_x,
            self.rect_y,
            self.settings.color,
            self.outline_color(),
            self.outline,
        );

        self.surface.flush();
        self.surface.present(self.hwnd);
    }

    /// 把自己重新插回置顶层的最前面。
    ///
    /// WS_EX_TOPMOST 是一个「层」，不是一个「第一位」：普通窗口永远翻不到它上面，
    /// 但层内部的顺序是按「谁最近被激活 / 最近插进来」排的。启动时插到层顶之后就
    /// 再也不管的话，后面陆续冒出来的置顶窗口（别的浮窗、输入法的 IME 窗口……）
    /// 会一个个插到前面，时间一长时钟就被挤到层底 —— 看着像「被别的窗口盖住了」。
    /// 点一下会激活它，激活本身就把窗口提回层顶，所以「点一下就恢复」。
    ///
    /// 所以要定期重申，把它按回层顶。重申的时机见 tick：只在「分」跳了的时候
    /// 做一次 —— 每秒都重申固然能把时钟一直压在层顶，但每秒动一次 z 序会打扰用户
    /// 正在进行的操作（拖别的窗口、全屏应用……），一分钟一次足够，真被盖住了点一下
    /// 时钟也会自己回到层顶。
    /// 两个 flag 都不能少：SWP_NOACTIVATE 是为了不抢焦点（重申置顶不该把用户
    /// 正在打字的窗口顶掉），SWP_NOMOVE|SWP_NOSIZE 是不让窗口挪地方。
    /// 关掉置顶的时候当然不能调，否则等于把用户的设置顶回去。
    fn keep_on_top(&self) {
        unsafe {
            if !self.settings.topmost || self.hwnd.is_invalid() {
                return;
            }
            let _ = SetWindowPos(
                self.hwnd,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
    }

    /// 每 TICK_MS 问一次时间，只在「秒」真的跳了才重画。
    /// 比死等 1000ms 稳：系统一忙定时器就会被拖后，靠 1 秒一跳会越走越慢。
    fn tick(&mut self) {
        let now = util::format_time();
        if now == self.time_text {
            return; // 秒还没跳，什么都不用做
        }

        // 重申置顶只在「分」跳了的时候做一次：秒每跳一下都动一次 z 序太吵，
        // 会打扰用户正在进行的操作（拖别的窗口、全屏应用……）。
        // 前 5 个字符就是 "HH:MM"，和上次重申时的比一比就知道分跳没跳。
        if now[..5] != self.topmost_stamp[..] {
            self.topmost_stamp = now[..5].to_string();
            self.keep_on_top();
        }

        self.time_text = now;
        self.render();
    }

    // ---------------------------------------------------------------- 对外

    pub fn create(reset_settings: bool) -> Option<Box<Clock>> {
        unsafe {
            let instance = GetModuleHandleW(None).ok()?;

            if reset_settings {
                settings::reset();
            }

            let mut clock = Box::new(Clock {
                hwnd: HWND::default(),
                surface: Surface::new(),
                font: None,
                settings: settings::load(),
                dpi_scale: util::dpi_scale(),
                font_px: 0.0,
                outline: 0,
                win_w: 0,
                win_h: 0,
                rect_x: 0.0,
                rect_y: 0.0,
                time_text: util::format_time(),
                topmost_stamp: String::new(),
                pressed: false,
                dragging: false,
                press_cursor: POINT::default(),
                press_origin: POINT::default(),
            });
            clock.topmost_stamp = clock.time_text[..5].to_string();

            let class_name = util::to_wide(WINDOW_CLASS);
            let title = util::to_wide(WINDOW_TITLE);

            // 四向箭头：光标一放上去就提示「这块能拖」
            let cursor = LoadCursorW(None, IDC_SIZEALL).ok()?;
            let window_class = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(clock_window_proc),
                hInstance: instance.into(),
                hCursor: cursor,
                hbrBackground: Default::default(), // 背景全透明，不留底色
                lpszClassName: PCWSTR(class_name.as_ptr()),
                ..Default::default()
            };
            if RegisterClassExW(&window_class) == 0 {
                return None;
            }

            let raw = Box::into_raw(clock);
            let ex_style = WS_EX_LAYERED
                | WS_EX_TOOLWINDOW
                | if (*raw).settings.topmost {
                    WS_EX_TOPMOST
                } else {
                    WINDOW_EX_STYLE(0)
                };
            let hwnd = CreateWindowExW(
                ex_style,
                PCWSTR(class_name.as_ptr()),
                PCWSTR(title.as_ptr()),
                WS_POPUP,
                0,
                0,
                16,
                16,
                None,
                None,
                Some(instance.into()),
                Some(raw.cast()),
            );
            let mut clock = Box::from_raw(raw);
            let hwnd = match hwnd {
                Ok(hwnd) if !hwnd.is_invalid() => hwnd,
                _ => {
                    let _ = UnregisterClassW(PCWSTR(class_name.as_ptr()), Some(instance.into()));
                    return None;
                }
            };
            clock.hwnd = hwnd;

            if !clock.relayout(false) {
                clock.destroy();
                return None;
            }

            clock.render();
            let _ = ShowWindow(clock.hwnd, SW_SHOWNOACTIVATE);
            SetTimer(Some(clock.hwnd), TIMER_TICK, TICK_MS, None);
            Some(clock)
        }
    }

    pub fn run(&self) {
        unsafe {
            // 消息循环本来就是全局的，留着 &self 只是为了接口看着对称
            let mut message = MSG::default();
            while GetMessageW(&mut message, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }

    pub fn destroy(mut self: Box<Clock>) {
        unsafe {
            if !self.hwnd.is_invalid() {
                let _ = KillTimer(Some(self.hwnd), TIMER_TICK);
                let _ = DestroyWindow(self.hwnd);
                self.hwnd = HWND::default();
            }
            let instance = GetModuleHandleW(None).unwrap_or_default();
            let class_name = util::to_wide(WINDOW_CLASS);
            let _ = UnregisterClassW(PCWSTR(class_name.as_ptr()), Some(instance.into()));
        }
        // Box 在这里析构：font、surface 的 Drop 在 GdiplusShutdown 之前跑完
    }

    // ---- 给右键菜单用的读写口 ----

    pub fn set_size_index(&mut self, index: i32) {
        let clamped = util::clamp_int(index, 0, SIZE_STEPS.len() as i32 - 1) as usize;
        if clamped == self.settings.size_index {
            return;
        }
        self.settings.size_index = clamped;
        if self.relayout(true) {
            self.render();
        }
        settings::save(&self.settings);
    }

    pub fn set_color(&mut self, argb: u32) {
        if argb == self.settings.color {
            return;
        }
        self.settings.color = argb;
        self.render();
        settings::save(&self.settings);
    }

    pub fn set_topmost(&mut self, on: bool) {
        self.settings.topmost = on;
        unsafe {
            let _ = SetWindowPos(
                self.hwnd,
                Some(if on { HWND_TOPMOST } else { HWND_NOTOPMOST }),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
        settings::save(&self.settings);
    }

    pub fn set_outline(&mut self, on: bool) {
        if self.settings.outline == on {
            return;
        }
        self.settings.outline = on;
        // 描边占地方：留白和墨迹范围都会变，得整套重排一遍
        if self.relayout(true) {
            self.render();
        }
        settings::save(&self.settings);
    }

    pub fn quit(&self) {
        unsafe {
            if !self.hwnd.is_invalid() {
                let _ = DestroyWindow(self.hwnd);
            }
        }
    }
}

// ---------------------------------------------------------------- 消息

unsafe extern "system" fn clock_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let clock: *mut Clock = if message == WM_NCCREATE {
        let create = lparam.0 as *const CREATESTRUCTW;
        let clock = (*create).lpCreateParams as *mut Clock;
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, clock as isize);
        (*clock).hwnd = hwnd;
        clock
    } else {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Clock;
        if ptr.is_null() {
            return DefWindowProcW(hwnd, message, wparam, lparam);
        }
        ptr
    };
    let clock = &mut *clock;

    match message {
        WM_TIMER => {
            if wparam.0 == TIMER_TICK {
                clock.tick();
            }
            LRESULT(0)
        }

        WM_LBUTTONDOWN => {
            let _ = SetForegroundWindow(hwnd); // 拿到前台，右键菜单和 Esc 才正常
            SetCapture(hwnd);
            let _ = GetCursorPos(&mut clock.press_cursor);
            let mut rect = RECT::default();
            let _ = GetWindowRect(hwnd, &mut rect);
            clock.press_origin.x = rect.left;
            clock.press_origin.y = rect.top;
            clock.pressed = true;
            clock.dragging = false;
            LRESULT(0)
        }

        WM_MOUSEMOVE => {
            if !clock.pressed {
                return LRESULT(0);
            }
            let mut cursor = POINT::default();
            let _ = GetCursorPos(&mut cursor);
            let dx = cursor.x - clock.press_cursor.x;
            let dy = cursor.y - clock.press_cursor.y;
            // 超过阈值才算拖动：手抖一下不该把时钟挪走
            if dx.abs() > DRAG_THRESHOLD || dy.abs() > DRAG_THRESHOLD {
                clock.dragging = true;
            }
            if clock.dragging {
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    clock.press_origin.x + dx,
                    clock.press_origin.y + dy,
                    0,
                    0,
                    SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
            LRESULT(0)
        }

        WM_LBUTTONUP => {
            // ReleaseCapture 会「同步」给本窗口发一条 WM_CAPTURECHANGED，
            // 那个分支会把 pressed/dragging 清掉，所以先把值取出来再松捕获。
            let was_pressed = clock.pressed;
            let was_dragging = clock.dragging;
            if GetCapture() == hwnd {
                let _ = ReleaseCapture();
            }
            clock.pressed = false;
            clock.dragging = false;
            if was_pressed && was_dragging {
                clock.remember_position();
            }
            LRESULT(0)
        }

        WM_CAPTURECHANGED => {
            clock.pressed = false;
            clock.dragging = false;
            LRESULT(0)
        }

        WM_RBUTTONUP => {
            menu::show(clock);
            LRESULT(0)
        }

        WM_MOUSEWHEEL => {
            // 滚轮调字号：不用去菜单里翻，顺手就能缩放
            let delta = ((wparam.0 >> 16) as i16) as i32;
            if delta > 0 {
                clock.set_size_index(clock.settings.size_index as i32 + 1);
            } else if delta < 0 {
                clock.set_size_index(clock.settings.size_index as i32 - 1);
            }
            LRESULT(0)
        }

        WM_KEYDOWN => {
            if wparam.0 == VK_ESCAPE.0 as usize {
                clock.quit();
                return LRESULT(0);
            }
            DefWindowProcW(hwnd, message, wparam, lparam)
        }

        WM_DISPLAYCHANGE => {
            // 换了显示器 / 改了分辨率：DPI 变了就得按新比例重排一遍
            let scale = util::dpi_scale();
            if scale != clock.dpi_scale {
                clock.dpi_scale = scale;
                if clock.relayout(true) {
                    clock.render();
                }
            }
            LRESULT(0)
        }

        WM_ERASEBKGND => LRESULT(1), // 分层窗口不靠 WM_PAINT，别擦背景

        WM_CLOSE => {
            clock.quit();
            LRESULT(0)
        }

        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }

        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}
