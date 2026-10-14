#include "clock.h"

#include <stdlib.h>
#include <string.h>
#include <wchar.h>

#include "config.h"
#include "menu.h"
#include "render.h"
#include "settings.h"
#include "surface.h"
#include "util.h"

#define K_TIMER_TICK 1

static const wchar_t kWindowClass[] = L"ClockDisplayWindow";
static const wchar_t kWindowTitle[] = L"桌面时钟";

/* 量排版时用的替身：全 8 的等宽字符串。
 * Consolas 这类等宽字体里，每个数字和冒号的步进宽度完全一样，所以「88:88:88」
 * 占的地方和任意时刻的真实时间分毫不差 —— 拿它定窗口大小，秒数再怎么跳，
 * 窗口也纹丝不动。 */
static const wchar_t kProbeText[] = L"88:88:88";

struct Clock {
    HINSTANCE instance;
    HWND hwnd;
    Surface surface;
    GpFont *font;
    Settings settings;

    double dpi_scale;
    float font_px;        /* 这一档字号实际用到的像素大小 */
    int outline;          /* 描边半径（像素） */
    int win_w, win_h;     /* 窗口 / 画布尺寸 */
    float rect_x, rect_y; /* DrawString 布局矩形的左上角 */

    wchar_t time_text[16];
    wchar_t topmost_stamp[6]; /* 上次重申置顶时的 "HH:MM"，分没跳就不重申 */

    int pressed;
    int dragging;
    POINT press_cursor;
    POINT press_origin;
};

/* ---------------------------------------------------------------- 位置 */

static void clock_work_area(RECT *work) {
    if (!SystemParametersInfoW(SPI_GETWORKAREA, 0, work, 0)) {
        work->left = 0;
        work->top = 0;
        work->right = GetSystemMetrics(SM_CXSCREEN);
        work->bottom = GetSystemMetrics(SM_CYSCREEN);
    }
}

/* 把窗口按回它所在那台显示器的可用区域里。
 * 换过显示器、改过分辨率之后，存档里的坐标可能已经落到屏幕外，
 * 不夹一下的话时钟就「消失」了 —— 明明在跑，就是看不见。 */
static void clock_clamp_position(int *x, int *y, int w, int h) {
    POINT center;
    HMONITOR monitor;
    MONITORINFO info;

    center.x = *x + w / 2;
    center.y = *y + h / 2;
    monitor = MonitorFromPoint(center, MONITOR_DEFAULTTONEAREST);
    info.cbSize = sizeof(info);
    if (monitor == NULL || !GetMonitorInfoW(monitor, &info)) return;

    if (*x + w > info.rcWork.right) *x = info.rcWork.right - w;
    if (*y + h > info.rcWork.bottom) *y = info.rcWork.bottom - h;
    if (*x < info.rcWork.left) *x = info.rcWork.left;
    if (*y < info.rcWork.top) *y = info.rcWork.top;
}

static void clock_remember_position(Clock *clock) {
    RECT rect;

    if (clock->hwnd == NULL) return;
    GetWindowRect(clock->hwnd, &rect);
    clock->settings.x = rect.left;
    clock->settings.y = rect.top;
    settings_save(&clock->settings);
}

/* 亮字配黑描边、暗字配白描边，压在任何壁纸上都读得清。 */
static unsigned int clock_outline_color(const Clock *clock) {
    return util_color_is_light(clock->settings.color) ? 0xFF000000u : 0xFFFFFFFFu;
}

/* ---------------------------------------------------------------- 排版 */

/* 画一遍、扫一遍，量出窗口该多大、文字该摆在哪儿。
 *
 * 为什么不用 GdipMeasureString 的结果当窗口尺寸：它量出来的是「行框」，
 * 上伸部和下伸部都留着空档 —— 数字既够不到顶、也踩不到底，照着裁出来的窗口
 * 上面紧、下面松，一眼就看出是歪的；而且它量出来的框还比字本身大一圈。
 * 所以这里改成真画一次，再扫 alpha 找出墨迹的真实范围（描边也算在内），拿它反推。 */
static int clock_measure_layout(Clock *clock, int *win_w, int *win_h, float *rect_x,
                                float *rect_y) {
    int pad;
    int probe_w;
    int probe_h;
    float origin_x;
    float origin_y;
    RECT ink;

    /* 墨迹四周留白：描边半径 + 一点呼吸空间，别让描边贴着窗口边被切掉 */
    pad = clock->outline + (int)(clock->font_px / 32.0f);
    if (pad < clock->outline + K_PAD_MIN) pad = clock->outline + K_PAD_MIN;

    /* 临时画布开得足够放肆：8 个字符最宽也占不到 5 个字宽，这边给了 10 个
     * （高度 3 个），再多的富余也只是临时位图，不花什么代价，却能保证
     * 「88:88:88」一次画全、不被画布边缘切掉。 */
    probe_w = (int)clock->font_px * 10 + 64;
    probe_h = (int)clock->font_px * 3 + 64;
    if (!surface_ensure(&clock->surface, probe_w, probe_h)) return 0;

    /* 排版矩形固定 K_TEXT_RECT_SIZE 见方，让它的中心落在临时画布正中 ——
     * 这样文字就居中画在画布上，扫出来的墨迹稳稳落在框内。 */
    origin_x = (float)probe_w / 2.0f - K_TEXT_RECT_SIZE / 2.0f;
    origin_y = (float)probe_h / 2.0f - K_TEXT_RECT_SIZE / 2.0f;

    surface_clear(&clock->surface);
    render_text(clock->surface.graphics, kProbeText, clock->font, origin_x, origin_y,
                clock->settings.color, clock_outline_color(clock), clock->outline);
    surface_flush(&clock->surface);
    if (!render_ink_bounds(clock->surface.bits, probe_w, probe_h, &ink)) return 0;

    *win_w = (ink.right - ink.left + 1) + pad * 2;
    *win_h = (ink.bottom - ink.top + 1) + pad * 2;

    /* 矩形尺寸是定死的，所以「矩形左上角挪多少，墨迹就跟着挪多少」是个线性关系：
     * 想让墨迹落在 (pad, pad)，把这次的左上角平移同样的差值即可。 */
    *rect_x = origin_x + (float)(pad - ink.left);
    *rect_y = origin_y + (float)(pad - ink.top);
    return 1;
}

/* 按当前字号重排，并把窗口摆好。
 * keep_center = 1 时以文字中心为锚点（改字号别让时钟蹦到别处去）。 */
static int clock_relayout(Clock *clock, int keep_center) {
    int width = 0;
    int height = 0;
    int x;
    int y;
    float rect_x = 0.0f;
    float rect_y = 0.0f;

    clock->font_px = (float)kSizeSteps[clock->settings.size_index] * (float)clock->dpi_scale;
    /* 描边半径按字号等比缩放；关掉的时候就是 0 —— 少了这一步的话，
     * 菜单里的勾选、存档、读档全都好好的，只有画面永远带着描边。 */
    if (clock->settings.outline) {
        clock->outline = (int)(clock->font_px / K_OUTLINE_DIVISOR + 0.5f);
        if (clock->outline < 1) clock->outline = 1;
    } else {
        clock->outline = 0;
    }

    render_delete_font(clock->font);
    clock->font = render_create_font(clock->font_px);
    if (clock->font == NULL) return 0;

    if (!clock_measure_layout(clock, &width, &height, &rect_x, &rect_y)) return 0;

    clock->win_w = width;
    clock->win_h = height;
    clock->rect_x = rect_x;
    clock->rect_y = rect_y;

    if (keep_center && clock->hwnd != NULL) {
        RECT rect;
        GetWindowRect(clock->hwnd, &rect);
        x = rect.left + (rect.right - rect.left) / 2 - width / 2;
        y = rect.top + (rect.bottom - rect.top) / 2 - height / 2;
    } else {
        x = clock->settings.x;
        y = clock->settings.y;
        if (x == K_POS_UNSET || y == K_POS_UNSET) {
            /* 头一回运行：水平居中，纵向离工作区顶部一点距离 */
            RECT work;
            clock_work_area(&work);
            x = work.left + (work.right - work.left - width) / 2;
            y = work.top + (int)((work.bottom - work.top) * K_START_TOP_RATIO);
        }
    }

    clock_clamp_position(&x, &y, width, height);
    clock->settings.x = x;
    clock->settings.y = y;

    SetWindowPos(clock->hwnd, clock->settings.topmost ? HWND_TOPMOST : HWND_NOTOPMOST, x, y,
                 width, height, SWP_NOACTIVATE);
    return 1;
}

/* ---------------------------------------------------------------- 绘制 */

static void clock_render(Clock *clock) {
    GpGraphics *graphics;

    if (clock->hwnd == NULL || clock->font == NULL) return;
    if (!surface_ensure(&clock->surface, clock->win_w, clock->win_h)) return;

    graphics = clock->surface.graphics;
    if (graphics == NULL) return;

    surface_clear(&clock->surface);
    GdipSetSmoothingMode(graphics, GP_SMOOTHING_ANTIALIAS);
    GdipSetPixelOffsetMode(graphics, GP_PIXEL_OFFSET_HALF);
    /* 分层窗口开不了 ClearType：次像素抗锯齿得靠不透明的底色，而这儿的底色
     * 是全透明的。灰阶抗锯齿（AntiAlias）配 alpha 通道才是正解 ——
     * 隐私屏幕那只钟用的是 CLEARTYPE_QUALITY，但它是画在不透明黑底上的。 */
    GdipSetTextRenderingHint(graphics, GP_TEXT_HINT_ANTIALIAS);

    render_text(graphics, clock->time_text, clock->font, clock->rect_x, clock->rect_y,
                clock->settings.color, clock_outline_color(clock), clock->outline);

    surface_flush(&clock->surface);
    surface_present(&clock->surface, clock->hwnd);
}

/* 把自己重新插回置顶层的最前面。
 *
 * WS_EX_TOPMOST 是一个「层」，不是一个「第一位」：普通窗口永远翻不到它上面，
 * 但层内部的顺序是按「谁最近被激活 / 最近插进来」排的。启动时插到层顶之后就
 * 再也不管的话，后面陆续冒出来的置顶窗口（别的浮窗、输入法的 IME 窗口……）
 * 会一个个插到前面，时间一长时钟就被挤到层底 —— 看着像「被别的窗口盖住了」。
 * 点一下会激活它，激活本身就把窗口提回层顶，所以「点一下就恢复」。
 *
 * 所以要定期重申，把它按回层顶。重申的时机见 clock_tick：只在「分」跳了的时候
 * 做一次 —— 每秒都重申固然能把时钟一直压在层顶，但每秒动一次 z 序会打扰用户
 * 正在进行的操作（拖别的窗口、全屏应用……），一分钟一次足够，真被盖住了点一下
 * 时钟也会自己回到层顶。
 * 两个 flag 都不能少：SWP_NOACTIVATE 是为了不抢焦点（重申置顶不该把用户
 * 正在打字的窗口顶掉），SWP_NOMOVE|SWP_NOSIZE 是不让窗口挪地方。
 * 关掉置顶的时候当然不能调，否则等于把用户的设置顶回去。 */
static void clock_keep_on_top(Clock *clock) {
    if (!clock->settings.topmost || clock->hwnd == NULL) return;
    SetWindowPos(clock->hwnd, HWND_TOPMOST, 0, 0, 0, 0,
                 SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
}

/* 每 K_TICK_MS 问一次时间，只在「秒」真的跳了才重画。
 * 比死等 1000ms 稳：系统一忙定时器就会被拖后，靠 1 秒一跳会越走越慢。 */
static void clock_tick(Clock *clock) {
    wchar_t now[16];

    util_format_time(now, (int)(sizeof(now) / sizeof(now[0])));
    if (wcscmp(now, clock->time_text) == 0) return; /* 秒还没跳，什么都不用做 */

    wcscpy(clock->time_text, now);

    /* 重申置顶只在「分」跳了的时候做一次：秒每跳一下都动一次 z 序太吵，
     * 会打扰用户正在进行的操作（拖别的窗口、全屏应用……）。
     * 前 5 个字符就是 "HH:MM"，和上次重申时的比一比就知道分跳没跳。 */
    if (wcsncmp(now, clock->topmost_stamp, 5) != 0) {
        memcpy(clock->topmost_stamp, now, 5 * sizeof(wchar_t));
        clock->topmost_stamp[5] = L'\0';
        clock_keep_on_top(clock);
    }

    clock_render(clock);
}

/* ---------------------------------------------------------------- 消息 */

static LRESULT CALLBACK clock_window_proc(HWND hwnd, UINT message, WPARAM wparam,
                                          LPARAM lparam) {
    Clock *clock;

    if (message == WM_NCCREATE) {
        CREATESTRUCTW *create = (CREATESTRUCTW *)lparam;
        clock = (Clock *)create->lpCreateParams;
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, (LONG_PTR)clock);
        clock->hwnd = hwnd;
    } else {
        clock = (Clock *)GetWindowLongPtrW(hwnd, GWLP_USERDATA);
        if (clock == NULL) return DefWindowProcW(hwnd, message, wparam, lparam);
    }

    switch (message) {
        case WM_TIMER:
            if (wparam == K_TIMER_TICK) clock_tick(clock);
            return 0;

        case WM_LBUTTONDOWN: {
            RECT rect;
            SetForegroundWindow(hwnd); /* 拿到前台，右键菜单和 Esc 才正常 */
            SetCapture(hwnd);
            GetCursorPos(&clock->press_cursor);
            GetWindowRect(hwnd, &rect);
            clock->press_origin.x = rect.left;
            clock->press_origin.y = rect.top;
            clock->pressed = 1;
            clock->dragging = 0;
            return 0;
        }

        case WM_MOUSEMOVE: {
            POINT cursor;
            int dx;
            int dy;

            if (!clock->pressed) return 0;
            GetCursorPos(&cursor);
            dx = cursor.x - clock->press_cursor.x;
            dy = cursor.y - clock->press_cursor.y;
            /* 超过阈值才算拖动：手抖一下不该把时钟挪走 */
            if (abs(dx) > K_DRAG_THRESHOLD || abs(dy) > K_DRAG_THRESHOLD) clock->dragging = 1;
            if (clock->dragging) {
                SetWindowPos(hwnd, NULL, clock->press_origin.x + dx,
                             clock->press_origin.y + dy, 0, 0,
                             SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
            }
            return 0;
        }

        case WM_LBUTTONUP: {
            /* ReleaseCapture 会「同步」给本窗口发一条 WM_CAPTURECHANGED，
             * 那个分支会把 pressed/dragging 清掉，所以先把值取出来再松捕获。 */
            const int was_pressed = clock->pressed;
            const int was_dragging = clock->dragging;
            if (GetCapture() == hwnd) ReleaseCapture();
            clock->pressed = 0;
            clock->dragging = 0;
            if (was_pressed && was_dragging) clock_remember_position(clock);
            return 0;
        }

        case WM_CAPTURECHANGED:
            clock->pressed = 0;
            clock->dragging = 0;
            return 0;

        case WM_RBUTTONUP:
            menu_show(clock);
            return 0;

        case WM_MOUSEWHEEL: {
            /* 滚轮调字号：不用去菜单里翻，顺手就能缩放 */
            const int delta = GET_WHEEL_DELTA_WPARAM(wparam);
            if (delta > 0) clock_set_size_index(clock, clock->settings.size_index + 1);
            else if (delta < 0) clock_set_size_index(clock, clock->settings.size_index - 1);
            return 0;
        }

        case WM_KEYDOWN:
            if (wparam == VK_ESCAPE) {
                clock_quit(clock);
                return 0;
            }
            break;

        case WM_DISPLAYCHANGE: {
            /* 换了显示器 / 改了分辨率：DPI 变了就得按新比例重排一遍 */
            const double scale = util_dpi_scale();
            if (scale != clock->dpi_scale) {
                clock->dpi_scale = scale;
                if (clock_relayout(clock, 1)) clock_render(clock);
            }
            return 0;
        }

        case WM_ERASEBKGND:
            return 1; /* 分层窗口不靠 WM_PAINT，别擦背景 */

        case WM_CLOSE:
            clock_quit(clock);
            return 0;

        case WM_DESTROY:
            PostQuitMessage(0);
            return 0;

        default:
            break;
    }

    return DefWindowProcW(hwnd, message, wparam, lparam);
}

/* ---------------------------------------------------------------- 对外 */

Clock *clock_create(HINSTANCE instance, int reset_settings) {
    Clock *clock;
    WNDCLASSEXW window_class;

    clock = (Clock *)calloc(1, sizeof(Clock));
    if (clock == NULL) return NULL;

    clock->instance = instance;
    clock->settings.x = K_POS_UNSET;
    clock->settings.y = K_POS_UNSET;

    if (reset_settings) settings_reset();
    settings_load(&clock->settings);
    clock->dpi_scale = util_dpi_scale();
    util_format_time(clock->time_text, (int)(sizeof(clock->time_text) / sizeof(wchar_t)));

    ZeroMemory(&window_class, sizeof(window_class));
    window_class.cbSize = sizeof(window_class);
    window_class.lpfnWndProc = clock_window_proc;
    window_class.hInstance = instance;
    /* 四向箭头：光标一放上去就提示「这块能拖」 */
    window_class.hCursor = LoadCursorW(NULL, IDC_SIZEALL);
    window_class.lpszClassName = kWindowClass;
    window_class.hbrBackground = NULL; /* 背景全透明，不留底色 */
    if (RegisterClassExW(&window_class) == 0) {
        free(clock);
        return NULL;
    }

    clock->hwnd = CreateWindowExW(
        WS_EX_LAYERED | WS_EX_TOOLWINDOW | (clock->settings.topmost ? WS_EX_TOPMOST : 0),
        kWindowClass, kWindowTitle, WS_POPUP, 0, 0, 16, 16, NULL, NULL, instance, clock);
    if (clock->hwnd == NULL) {
        UnregisterClassW(kWindowClass, instance);
        free(clock);
        return NULL;
    }

    if (!clock_relayout(clock, 0)) {
        clock_destroy(clock);
        return NULL;
    }

    clock_render(clock);
    ShowWindow(clock->hwnd, SW_SHOWNOACTIVATE);
    SetTimer(clock->hwnd, K_TIMER_TICK, K_TICK_MS, NULL);
    return clock;
}

void clock_run(Clock *clock) {
    MSG message;

    (void)clock; /* 消息循环本来就是全局的，留着参数只是为了接口看着对称 */

    while (GetMessageW(&message, NULL, 0, 0) > 0) {
        TranslateMessage(&message);
        DispatchMessageW(&message);
    }
}

void clock_destroy(Clock *clock) {
    if (clock == NULL) return;

    if (clock->hwnd != NULL) {
        KillTimer(clock->hwnd, K_TIMER_TICK);
        DestroyWindow(clock->hwnd);
        clock->hwnd = NULL;
    }
    UnregisterClassW(kWindowClass, clock->instance);

    render_delete_font(clock->font);
    surface_release(&clock->surface);
    free(clock);
}

HWND clock_window(const Clock *clock) {
    return clock->hwnd;
}

int clock_size_index(const Clock *clock) {
    return clock->settings.size_index;
}

unsigned int clock_color(const Clock *clock) {
    return clock->settings.color;
}

int clock_topmost(const Clock *clock) {
    return clock->settings.topmost;
}

int clock_outline(const Clock *clock) {
    return clock->settings.outline;
}

void clock_set_size_index(Clock *clock, int index) {
    const int clamped = util_clamp_int(index, 0, kSizeStepCount - 1);
    if (clamped == clock->settings.size_index) return;

    clock->settings.size_index = clamped;
    if (clock_relayout(clock, 1)) clock_render(clock);
    settings_save(&clock->settings);
}

void clock_set_color(Clock *clock, unsigned int argb) {
    if (argb == clock->settings.color) return;

    clock->settings.color = argb;
    clock_render(clock);
    settings_save(&clock->settings);
}

void clock_set_topmost(Clock *clock, int on) {
    clock->settings.topmost = on ? 1 : 0;
    SetWindowPos(clock->hwnd, clock->settings.topmost ? HWND_TOPMOST : HWND_NOTOPMOST, 0, 0, 0,
                 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
    settings_save(&clock->settings);
}

void clock_set_outline(Clock *clock, int on) {
    if (clock->settings.outline == (on ? 1 : 0)) return;

    clock->settings.outline = on ? 1 : 0;
    /* 描边占地方：留白和墨迹范围都会变，得整套重排一遍 */
    if (clock_relayout(clock, 1)) clock_render(clock);
    settings_save(&clock->settings);
}

void clock_quit(Clock *clock) {
    if (clock->hwnd != NULL) DestroyWindow(clock->hwnd);
}
