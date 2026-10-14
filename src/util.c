#include "util.h"

#include <stdio.h>
#include <wchar.h>

double util_dpi_scale(void) {
    HDC screen = GetDC(NULL);
    int dpi = (screen != NULL) ? GetDeviceCaps(screen, LOGPIXELSX) : 96;
    if (screen != NULL) ReleaseDC(NULL, screen);
    if (dpi <= 0) dpi = 96;
    return (double)dpi / 96.0;
}

void util_format_time(wchar_t *buffer, int capacity) {
    SYSTEMTIME now;

    if (capacity < 9) { /* "HH:MM:SS" + 结尾的 0 */
        if (capacity > 0) buffer[0] = L'\0';
        return;
    }

    GetLocalTime(&now);
    swprintf(buffer, (size_t)capacity, L"%02d:%02d:%02d", (int)now.wHour, (int)now.wMinute,
             (int)now.wSecond);
}

int util_color_is_light(unsigned int argb) {
    /* 人眼对绿最敏感，红蓝次之 —— 加权亮度过半就算亮色 */
    const int r = (int)((argb >> 16) & 0xFFu);
    const int g = (int)((argb >> 8) & 0xFFu);
    const int b = (int)(argb & 0xFFu);
    return (r * 299 + g * 587 + b * 114) / 1000 >= 128;
}

int util_clamp_int(int value, int low, int high) {
    if (value < low) return low;
    if (value > high) return high;
    return value;
}
