#pragma once

#include <windows.h>

/* 系统 DPI 缩放：96 DPI = 1.0。高 DPI 屏上字号和留白按这个等比放大，
 * 不然 4K 屏上会小得像蚂蚁。 */
double util_dpi_scale(void);

/* 当前本地时间的 "HH:MM:SS"（24 小时制、补零），格式和隐私屏幕那只钟一致。 */
void util_format_time(wchar_t *buffer, int capacity);

/* 这个颜色算不算「亮色」（0xAARRGGBB）。
 * 亮色文字配黑描边、暗色文字配白描边，压在任何壁纸上都读得清。 */
int util_color_is_light(unsigned int argb);

int util_clamp_int(int value, int low, int high);
