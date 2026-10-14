/* ===========================================================================
 *  桌面时钟 —— C / Win32 / GDI+
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
 *  设置存在 exe 同目录的 clock.ini 里。
 * ======================================================================== */

#include <windows.h>

#include <shellapi.h> /* CommandLineToArgvW */
#include <string.h>
#include <wchar.h>

#include "clock.h"
#include "gdiplus_c.h"
#include "render.h"

static const wchar_t kAppTitle[] = L"桌面时钟";

typedef struct {
    int reset; /* --reset：把存档删掉，回到默认位置/字号/颜色 */
    int help;
} Options;

static Options parse_command_line(void) {
    Options options;
    int argc = 0;
    int i;
    LPWSTR *argv;

    options.reset = 0;
    options.help = 0;

    argv = CommandLineToArgvW(GetCommandLineW(), &argc);
    if (argv == NULL) return options;

    for (i = 1; i < argc; ++i) {
        const wchar_t *argument = argv[i];
        if (wcscmp(argument, L"--reset") == 0 || wcscmp(argument, L"-r") == 0) {
            options.reset = 1;
        } else if (wcscmp(argument, L"--help") == 0 || wcscmp(argument, L"-h") == 0 ||
                   wcscmp(argument, L"/?") == 0) {
            options.help = 1;
        }
    }

    LocalFree(argv);
    return options;
}

/* 打包成窗口程序（没有控制台）之后，printf 是没人看的，要跟用户说话只能弹框。
 * 帮助就是这么给的。 */
static void show_help(void) {
    MessageBoxW(
        NULL,
        L"桌面时钟 —— 屏幕上只剩一行时间，背景全透明\n"
        L"\n"
        L"用法：clock_display.exe [选项]\n"
        L"\n"
        L"  -r, --reset    恢复默认位置、字号和颜色\n"
        L"                 （时钟被拖到屏幕外找不回来时用这个）\n"
        L"  -h, --help     显示这份帮助\n"
        L"\n"
        L"操作\n"
        L"  左键拖动    移动时钟（只有文字本身点得中，透明处鼠标会穿过去）\n"
        L"  鼠标滚轮    调字号\n"
        L"  右键        菜单：颜色 / 字号 / 描边 / 置顶 / 关闭\n"
        L"  Esc         关闭\n"
        L"\n"
        L"设置自动存在 exe 同目录的 clock.ini 里。",
        kAppTitle, MB_OK | MB_ICONINFORMATION);
}

int WINAPI WinMain(HINSTANCE instance, HINSTANCE previous, LPSTR command_line, int show) {
    GdiplusStartupInput startup_input;
    ULONG_PTR gdiplus_token = 0;
    Options options;
    Clock *clock;
    int exit_code = 0;

    (void)previous;
    (void)command_line;
    (void)show;

    options = parse_command_line();
    if (options.help) {
        show_help();
        return 0;
    }

    /* 清单里已经声明了 dpiAware，这里再兜一次底：万一清单没生效，
     * 高 DPI 下也不至于被系统拉成一团糊。 */
    SetProcessDPIAware();

    startup_input.GdiplusVersion = 1;
    startup_input.DebugEventCallback = NULL;
    startup_input.SuppressBackgroundThread = FALSE;
    startup_input.SuppressExternalCodecs = FALSE;
    if (GdiplusStartup(&gdiplus_token, &startup_input, NULL) != 0) {
        MessageBoxW(NULL, L"GDI+ 初始化失败，画不了字，程序退出。", kAppTitle,
                    MB_OK | MB_ICONERROR);
        return 2;
    }

    {
        /* 这个作用域很关键：所有 GDI+ 对象都得在 GdiplusShutdown 之前析构干净。 */
        if (render_family() == NULL) {
            MessageBoxW(NULL, L"系统里找不到任何能用的字体（Consolas、微软雅黑、Segoe UI 都没有），"
                              L"没法画时间。",
                        kAppTitle, MB_OK | MB_ICONERROR);
            exit_code = 2;
        } else {
            clock = clock_create(instance, options.reset);
            if (clock == NULL) {
                MessageBoxW(NULL, L"窗口创建失败。", kAppTitle, MB_OK | MB_ICONERROR);
                exit_code = 2;
            } else {
                clock_run(clock);
                clock_destroy(clock);
            }
        }
    }

    GdiplusShutdown(gdiplus_token);
    return exit_code;
}
