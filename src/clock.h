#pragma once

#include <windows.h>

/* 时钟本体：一个无边框、背景全透明的分层窗口，上面只画一行时间。
 *
 * 分层窗口在 alpha 为 0 的地方本来就把鼠标消息放行给下面的窗口，
 * 于是「只有文字能点中」是白送的 —— 不用自己去算命中区域，
 * 也就正好是想要的交互：点着文字能拖，透明处戳不着。 */
typedef struct Clock Clock;

/* 建窗口并显示。失败返回 NULL。 */
Clock *clock_create(HINSTANCE instance, int reset_settings);

void clock_run(Clock *clock);
void clock_destroy(Clock *clock);

/* ---- 给右键菜单用的读写口 ---- */
HWND clock_window(const Clock *clock);
int clock_size_index(const Clock *clock);
unsigned int clock_color(const Clock *clock);
int clock_topmost(const Clock *clock);
int clock_outline(const Clock *clock);

/* 下面几个都是「改完立刻生效并写进存档」。
 * 字号变化会重排（窗口跟着缩），而且以当前文字中心为锚点，不会跳到别处。 */
void clock_set_size_index(Clock *clock, int index);
void clock_set_color(Clock *clock, unsigned int argb);
void clock_set_topmost(Clock *clock, int on);
void clock_set_outline(Clock *clock, int on);

void clock_quit(Clock *clock);
