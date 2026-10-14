#pragma once

#include "clock.h"

/* 在光标处弹右键菜单：颜色 / 字号 / 描边 / 置顶 / 关闭。
 * 菜单是模态的（TrackPopupMenu 不返回，消息循环都停在这儿），选完当场生效。 */
void menu_show(Clock *clock);
