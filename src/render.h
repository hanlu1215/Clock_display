#pragma once

#include <windows.h>

#include "gdiplus_c.h"

/* 界面字体族（按 kFontChoices 顺序挑第一个装了的），取到就一直缓存着用。
 * 一个都取不到返回 NULL —— 这时程序没法画字，调用方应当直接报错退出。 */
GpFontFamily *render_family(void);

/* 按「像素」字号建一个粗体字（不是磅值），用完 render_delete_font 释放。 */
GpFont *render_create_font(float pixel_size);
void render_delete_font(GpFont *font);

/* 排版矩形固定开这么大（见 render.c 顶上的说明）。
 * 调用方拿它的一半来摆位：矩形左上角 = 想让文字中心在的位置 - 这个值的一半。 */
#define K_TEXT_RECT_SIZE 4000.0f

/* 画一行带描边的文字。
 *
 * rect_x / rect_y 是 DrawString 那个「排版矩形」的左上角；矩形尺寸固定是
 * K_TEXT_RECT_SIZE 见方，文字居中排在里面，八个方向的描边就是把这个矩形
 * 平移若干像素再画一遍。
 *
 * 居中的是「行框」不是墨迹：行框留着上伸部和下伸部的空档，数字既碰不到顶也
 * 踩不到底，所以看着会略偏上。墨迹的准确位置得靠 render_ink_bounds 量出来反推，
 * 好在矩形尺寸固定 → 矩形挪多少，墨迹就挪多少，平移量是线性的。 */
void render_text(GpGraphics *graphics, const wchar_t *text, GpFont *font, float rect_x,
                 float rect_y, unsigned int color, unsigned int outline_color, int outline);

/* 扫一遍画布，量出真正画上去的墨迹范围（含描边）。
 * bits 是 32bpp 预乘 BGRA，alpha 不够 K_INK_ALPHA_MIN 的算没画。
 * 一个像素都没画到就返回 0，bounds 不动。 */
int render_ink_bounds(const BYTE *bits, int width, int height, RECT *bounds);
