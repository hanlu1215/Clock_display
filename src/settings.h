#pragma once

#include <windows.h>

/* 存在 exe 旁边的 clock.ini 里的那点设置。
 * 时钟很小，设置也就这么几项，用 Win32 自带的 INI 接口读写最省事。 */
typedef struct {
    unsigned int color; /* 0xAARRGGBB */
    int size_index;     /* kSizeSteps 的下标 */
    int x;              /* 窗口左上角的屏幕坐标 */
    int y;              /* 还没定过就是 K_POS_UNSET */
    int topmost;        /* 是否置顶 */
    int outline;        /* 是否描边 */
} Settings;

void settings_defaults(Settings *settings);

/* 读存档：文件不在、键缺了、值离谱，都退回默认值，绝不因为存档坏了起不来。 */
void settings_load(Settings *settings);

/* 写存档。写在 Program Files 这类没权限的地方会失败，静默忽略即可。 */
void settings_save(const Settings *settings);

/* --reset：把存档删掉，下次启动就是全新状态。
 * （把时钟拖到屏幕外找不回来时的救命选项。） */
void settings_reset(void);
