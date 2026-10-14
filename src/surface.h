#pragma once

#include <windows.h>

#include "gdiplus_c.h"

/* ===========================================================================
 *  一块 32bpp 预乘 alpha 的内存画布，外加一个 GDI+ 画布。
 *
 *  UpdateLayeredWindow(ULW_ALPHA) 要的就是这个格式：逐像素透明度。
 *  文字的抗锯齿边缘是按 alpha 混合上去的，所以不会有「色键抠图」那种
 *  一圈杂色的锯齿边 —— 这正是要透明背景时钟必须走这条路的原因。
 * ======================================================================== */
typedef struct {
    int width;
    int height;
    HDC dc;            /* 内存 DC，位图已选入 */
    HBITMAP dib;       /* 32bpp 自上而下的 DIB */
    HGDIOBJ old_bitmap;
    BYTE *bits;        /* DIB 的像素：每像素 BGRA，alpha 已预乘，行距 width*4 */
    GpBitmap *bitmap;  /* 把 bits 直接包成 GDI+ 位图，画完即所见 */
    GpGraphics *graphics;
} Surface;

/* 尺寸没变就复用（内容不保证保留，重新画之前一律先 surface_clear）。
 * 返回 0 表示失败。 */
int surface_ensure(Surface *surface, int width, int height);

void surface_release(Surface *surface);

/* 整块清成全透明。 */
void surface_clear(Surface *surface);

/* 等 GDI+ 把这一批绘制真正落到 bits 上。
 * 想直接读 bits（比如量文字墨迹）之前必须先调它。 */
void surface_flush(Surface *surface);

/* 把画布内容整体推给分层窗口；位置不传，只更新内容。
 * 传 nullptr 窗口或者尺寸对不上都返回 0。 */
int surface_present(Surface *surface, HWND window);
