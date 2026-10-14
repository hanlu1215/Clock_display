#include "settings.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <wchar.h>

#include "config.h"
#include "util.h"

/* 存档路径 = exe 所在目录 + clock.ini。
 * 之所以跟着 exe 而不是放 %APPDATA%：绿色软件，整个文件夹拷到别的机器上，
 * 位置、颜色、字号一起跟着走。 */
static void settings_path(wchar_t *buffer, int capacity) {
    DWORD length;
    wchar_t *slash;

    buffer[0] = L'\0';

    length = GetModuleFileNameW(NULL, buffer, (DWORD)capacity);
    if (length == 0 || length >= (DWORD)capacity) {
        buffer[0] = L'\0';
        return;
    }

    slash = wcsrchr(buffer, L'\\'); /* 砍掉文件名，只留目录 */
    if (slash == NULL) {
        buffer[0] = L'\0';
        return;
    }
    slash[1] = L'\0';

    if (wcslen(buffer) + wcslen(K_INI_NAME) + 1 > (size_t)capacity) {
        buffer[0] = L'\0';
        return;
    }
    wcscat(buffer, K_INI_NAME);
}

static int ini_int(const wchar_t *path, const wchar_t *key, int fallback) {
    wchar_t text[64];

    text[0] = L'\0';
    GetPrivateProfileStringW(K_INI_SECTION, key, L"", text,
                             (DWORD)(sizeof(text) / sizeof(text[0])), path);
    if (text[0] == L'\0') return fallback;

    /* 用 wcstol 而不是 GetPrivateProfileIntW：副屏摆在主屏左边时坐标是负数，
     * 那个接口只认无符号数。 */
    return (int)wcstol(text, NULL, 10);
}

static unsigned int ini_hex(const wchar_t *path, const wchar_t *key,
                            unsigned int fallback) {
    wchar_t text[64];

    text[0] = L'\0';
    GetPrivateProfileStringW(K_INI_SECTION, key, L"", text,
                             (DWORD)(sizeof(text) / sizeof(text[0])), path);
    if (text[0] == L'\0') return fallback;
    return (unsigned int)wcstoul(text, NULL, 16);
}

static void ini_write(const wchar_t *path, const wchar_t *key, const wchar_t *value) {
    WritePrivateProfileStringW(K_INI_SECTION, key, value, path);
}

void settings_defaults(Settings *settings) {
    settings->color = K_DEFAULT_COLOR;
    settings->size_index = K_DEFAULT_SIZE_INDEX;
    settings->x = K_POS_UNSET;
    settings->y = K_POS_UNSET;
    settings->topmost = K_DEFAULT_TOPMOST;
    settings->outline = K_DEFAULT_OUTLINE;
}

void settings_load(Settings *settings) {
    wchar_t path[MAX_PATH];
    unsigned int color;

    settings_defaults(settings);

    settings_path(path, (int)(sizeof(path) / sizeof(path[0])));
    if (path[0] == L'\0') return; /* 连自己在哪儿都不知道，就用默认值 */

    settings->size_index = util_clamp_int(ini_int(path, L"size", K_DEFAULT_SIZE_INDEX), 0,
                                          kSizeStepCount - 1);
    settings->x = ini_int(path, L"x", K_POS_UNSET);
    settings->y = ini_int(path, L"y", K_POS_UNSET);
    settings->topmost = ini_int(path, L"topmost", K_DEFAULT_TOPMOST) ? 1 : 0;
    settings->outline = ini_int(path, L"outline", K_DEFAULT_OUTLINE) ? 1 : 0;

    color = ini_hex(path, L"color", K_DEFAULT_COLOR);
    if ((color >> 24) == 0u) color |= 0xFF000000u; /* 存档里没带 alpha 就补成不透明 */
    settings->color = color;
}

void settings_save(const Settings *settings) {
    wchar_t path[MAX_PATH];
    wchar_t text[64];

    settings_path(path, (int)(sizeof(path) / sizeof(path[0])));
    if (path[0] == L'\0') return;

    /* 先把整节删掉再写：存档里不会留下上一版程序的旧键。 */
    WritePrivateProfileStringW(K_INI_SECTION, NULL, NULL, path);

    swprintf(text, (size_t)(sizeof(text) / sizeof(text[0])), L"%08X", settings->color);
    ini_write(path, L"color", text);
    swprintf(text, (size_t)(sizeof(text) / sizeof(text[0])), L"%d", settings->size_index);
    ini_write(path, L"size", text);
    swprintf(text, (size_t)(sizeof(text) / sizeof(text[0])), L"%d", settings->x);
    ini_write(path, L"x", text);
    swprintf(text, (size_t)(sizeof(text) / sizeof(text[0])), L"%d", settings->y);
    ini_write(path, L"y", text);
    ini_write(path, L"topmost", settings->topmost ? L"1" : L"0");
    ini_write(path, L"outline", settings->outline ? L"1" : L"0");
}

void settings_reset(void) {
    wchar_t path[MAX_PATH];

    settings_path(path, (int)(sizeof(path) / sizeof(path[0])));
    if (path[0] == L'\0') return;
    DeleteFileW(path); /* 删不掉（没有写权限之类）也无所谓，下次照样按存档跑 */
}
