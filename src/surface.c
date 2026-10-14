#include "surface.h"

int surface_ensure(Surface *surface, int width, int height) {
    BITMAPINFO info;
    void *bits = NULL;
    HBITMAP dib;
    HDC dc;

    if (width <= 0 || height <= 0) return 0;
    if (surface->graphics != NULL && width == surface->width && height == surface->height) {
        return 1;
    }

    surface_release(surface);

    ZeroMemory(&info, sizeof(info));
    info.bmiHeader.biSize = sizeof(BITMAPINFOHEADER);
    info.bmiHeader.biWidth = width;
    info.bmiHeader.biHeight = -height; /* 负数 = 自上而下，行序和 GDI+ 一致 */
    info.bmiHeader.biPlanes = 1;
    info.bmiHeader.biBitCount = 32;
    info.bmiHeader.biCompression = BI_RGB;

    dib = CreateDIBSection(NULL, &info, DIB_RGB_COLORS, &bits, NULL, 0);
    if (dib == NULL || bits == NULL) {
        if (dib != NULL) DeleteObject(dib);
        return 0;
    }

    dc = CreateCompatibleDC(NULL);
    if (dc == NULL) {
        DeleteObject(dib);
        return 0;
    }

    /* PixelFormat32bppPARGB：预乘 alpha，正好是 ULW_ALPHA 要的格式。
     * 这里把 DIB 的内存直接交给 GDI+ 当画布，所以画完 Flush 之后，
     * bits 里就是最终像素 —— 量墨迹、推给窗口都直接读它。 */
    if (GdipCreateBitmapFromScan0(width, height, width * 4, GP_PIXEL_FORMAT_32BPP_PARGB,
                                  (BYTE *)bits, &surface->bitmap) != 0) {
        DeleteDC(dc);
        DeleteObject(dib);
        surface->bitmap = NULL;
        return 0;
    }
    if (GdipGetImageGraphicsContext(surface->bitmap, &surface->graphics) != 0) {
        GdipDisposeImage(surface->bitmap);
        DeleteDC(dc);
        DeleteObject(dib);
        surface->bitmap = NULL;
        surface->graphics = NULL;
        return 0;
    }

    surface->width = width;
    surface->height = height;
    surface->dib = dib;
    surface->dc = dc;
    surface->bits = (BYTE *)bits;
    surface->old_bitmap = SelectObject(dc, dib);
    return 1;
}

void surface_release(Surface *surface) {
    /* 顺序要紧：GDI+ 对象先没，再还 GDI 位图，最后删 DC。 */
    if (surface->graphics != NULL) {
        GdipDeleteGraphics(surface->graphics);
        surface->graphics = NULL;
    }
    if (surface->bitmap != NULL) {
        GdipDisposeImage(surface->bitmap);
        surface->bitmap = NULL;
    }
    if (surface->dc != NULL && surface->old_bitmap != NULL) {
        SelectObject(surface->dc, surface->old_bitmap);
    }
    surface->old_bitmap = NULL;
    if (surface->dib != NULL) {
        DeleteObject(surface->dib);
        surface->dib = NULL;
    }
    if (surface->dc != NULL) {
        DeleteDC(surface->dc);
        surface->dc = NULL;
    }
    surface->bits = NULL;
    surface->width = 0;
    surface->height = 0;
}

void surface_clear(Surface *surface) {
    if (surface->graphics == NULL) return;
    GdipGraphicsClear(surface->graphics, 0x00000000u); /* 全透明 */
}

void surface_flush(Surface *surface) {
    if (surface->graphics == NULL) return;
    GdipFlush(surface->graphics, GP_FLUSH_SYNC);
}

int surface_present(Surface *surface, HWND window) {
    POINT source;
    SIZE size;
    BLENDFUNCTION blend;

    if (window == NULL || surface->dc == NULL) return 0;

    source.x = 0;
    source.y = 0;
    size.cx = surface->width;
    size.cy = surface->height;

    blend.BlendOp = AC_SRC_OVER;
    blend.BlendFlags = 0;
    blend.SourceConstantAlpha = 255;
    blend.AlphaFormat = AC_SRC_ALPHA;

    /* pptDst 传 NULL = 位置不动，只换内容。窗口在哪儿由 SetWindowPos 管，
     * 拖动的时候就不用每秒重推一次整块位图了。 */
    return UpdateLayeredWindow(window, NULL, NULL, &size, surface->dc, &source, 0, &blend,
                               ULW_ALPHA) != FALSE;
}
