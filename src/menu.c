#include "menu.h"

#include <commdlg.h>
#include <stdio.h>

#include "config.h"

/* 菜单项 id：颜色和字号各占一段连续区间，按下标反查选项。 */
#define K_MENU_COLOR_BASE 2000
#define K_MENU_SIZE_BASE 2100
#define K_MENU_COLOR_CUSTOM 2200
#define K_MENU_OUTLINE 2201
#define K_MENU_TOPMOST 2202
#define K_MENU_QUIT 2203

/* ChooseColorW 的「自定义颜色」格子：得跨次调用留着，所以放静态。
 * 不放的话每次打开颜色对话框，上次调好的颜色就没了。 */
static COLORREF s_custom_colors[16];

static HMENU build_color_menu(const Clock *clock) {
    const unsigned int current = clock_color(clock);
    HMENU menu = CreatePopupMenu();
    int matched = 0;
    int i;

    if (menu == NULL) return NULL;

    for (i = 0; i < kColorChoiceCount; ++i) {
        UINT flags = MF_STRING;
        if (kColorChoices[i].argb == current) {
            flags |= MF_CHECKED;
            matched = 1;
        }
        AppendMenuW(menu, flags, K_MENU_COLOR_BASE + i, kColorChoices[i].name);
    }

    AppendMenuW(menu, MF_SEPARATOR, 0, NULL);
    /* 当前颜色不在预设里，就说明是自己调的，把勾打在这一项上 */
    AppendMenuW(menu, MF_STRING | (matched ? MF_UNCHECKED : MF_CHECKED), K_MENU_COLOR_CUSTOM,
                L"自定义…");
    return menu;
}

static HMENU build_size_menu(const Clock *clock) {
    const int current = clock_size_index(clock);
    HMENU menu = CreatePopupMenu();
    wchar_t label[32];
    int i;

    if (menu == NULL) return NULL;

    for (i = 0; i < kSizeStepCount; ++i) {
        swprintf(label, (size_t)(sizeof(label) / sizeof(label[0])), L"%d 像素", kSizeSteps[i]);
        AppendMenuW(menu, MF_STRING | (i == current ? MF_CHECKED : MF_UNCHECKED),
                    K_MENU_SIZE_BASE + i, label);
    }
    return menu;
}

static void choose_custom_color(Clock *clock) {
    const unsigned int current = clock_color(clock);
    CHOOSECOLORW dialog;

    ZeroMemory(&dialog, sizeof(dialog));
    dialog.lStructSize = sizeof(dialog);
    dialog.hwndOwner = clock_window(clock);
    dialog.rgbResult = RGB((current >> 16) & 0xFFu, (current >> 8) & 0xFFu, current & 0xFFu);
    dialog.lpCustColors = s_custom_colors;
    dialog.Flags = CC_FULLOPEN | CC_RGBINIT;

    if (!ChooseColorW(&dialog)) return; /* 点了取消 */

    /* COLORREF 是 0x00BBGGRR，转成程序内部用的 0xAARRGGBB（并补上不透明的 alpha） */
    clock_set_color(clock, 0xFF000000u | ((unsigned int)GetRValue(dialog.rgbResult) << 16) |
                               ((unsigned int)GetGValue(dialog.rgbResult) << 8) |
                               (unsigned int)GetBValue(dialog.rgbResult));
}

static void handle_command(Clock *clock, int command) {
    if (command >= K_MENU_COLOR_BASE && command < K_MENU_COLOR_BASE + kColorChoiceCount) {
        clock_set_color(clock, kColorChoices[command - K_MENU_COLOR_BASE].argb);
        return;
    }
    if (command >= K_MENU_SIZE_BASE && command < K_MENU_SIZE_BASE + kSizeStepCount) {
        clock_set_size_index(clock, command - K_MENU_SIZE_BASE);
        return;
    }

    switch (command) {
        case K_MENU_COLOR_CUSTOM:
            choose_custom_color(clock);
            break;
        case K_MENU_OUTLINE:
            clock_set_outline(clock, !clock_outline(clock));
            break;
        case K_MENU_TOPMOST:
            clock_set_topmost(clock, !clock_topmost(clock));
            break;
        case K_MENU_QUIT:
            clock_quit(clock);
            break;
        default:
            break; /* 点在菜单外面关掉了：什么都不做 */
    }
}

void menu_show(Clock *clock) {
    HWND window = clock_window(clock);
    HMENU root;
    HMENU colors;
    HMENU sizes;
    POINT cursor;
    int command;

    root = CreatePopupMenu();
    colors = build_color_menu(clock);
    sizes = build_size_menu(clock);
    if (root == NULL || colors == NULL || sizes == NULL) {
        if (root != NULL) DestroyMenu(root);
        if (colors != NULL) DestroyMenu(colors);
        if (sizes != NULL) DestroyMenu(sizes);
        return;
    }

    AppendMenuW(root, MF_POPUP, (UINT_PTR)colors, L"颜色");
    AppendMenuW(root, MF_POPUP, (UINT_PTR)sizes, L"字号");
    AppendMenuW(root, MF_SEPARATOR, 0, NULL);
    AppendMenuW(root, MF_STRING | (clock_outline(clock) ? MF_CHECKED : MF_UNCHECKED),
                K_MENU_OUTLINE, L"描边");
    AppendMenuW(root, MF_STRING | (clock_topmost(clock) ? MF_CHECKED : MF_UNCHECKED),
                K_MENU_TOPMOST, L"置顶");
    AppendMenuW(root, MF_SEPARATOR, 0, NULL);
    AppendMenuW(root, MF_STRING, K_MENU_QUIT, L"关闭");

    GetCursorPos(&cursor);

    /* TrackPopupMenu 要求窗口在前台，否则菜单弹出来后点别处不会消失 */
    SetForegroundWindow(window);
    command = (int)TrackPopupMenu(root, TPM_RIGHTBUTTON | TPM_RETURNCMD | TPM_NONOTIFY, cursor.x,
                                  cursor.y, 0, window, NULL);
    DestroyMenu(root); /* 递归的，子菜单一起销毁 */

    /* 喂掉菜单留下的空消息，不然前台锁会卡住下一次菜单 */
    PostMessageW(window, WM_NULL, 0, 0);

    handle_command(clock, command);
}
