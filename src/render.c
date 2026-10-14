#include "render.h"

#include "config.h"

/* 为什么排版矩形要固定成一个「远大于文字」的尺寸，而不是用 GdipMeasureString
 * 量出来的框：
 *
 * GDI+ 量出来的框比字实际占的地方大一圈 —— 实测 Consolas 粗体 96px 下量
 * "88:88:88" 是 467×124，而真正画上去的墨迹只有 425×64。矩形一旦收紧到那个
 * 尺寸，GDI+ 居中放置的基准就开始飘：给一个 434 宽的矩形画 467 宽的排版框，
 * 整串字会往右偏二十几个像素，末尾那位数字直接被窗口切掉（踩过这个坑）。
 *
 * 固定成 4000×4000 之后，矩形的尺寸恒定，墨迹相对「矩形左上角」的偏移就是个常量，
 * 想让墨迹落在哪儿，把左上角平移同样多就行 —— 位置完全可算，不受字体和字号影响。 */

GpFontFamily *render_family(void) {
    /* 这个 family 是「故意泄漏」的，别改成在退出时释放：
     * 函数内静态对象要等 main 返回之后才析构，那会儿 GdiplusShutdown 早跑过了，
     * 再去析构一个 GDI+ 对象必崩在 CRT 的退出处理里。 */
    static GpFontFamily *family = NULL;
    static int tried = 0;
    int i;

    if (tried) return family;
    tried = 1;

    for (i = 0; i < kFontChoiceCount; ++i) {
        GpFontFamily *probe = NULL;
        if (GdipCreateFontFamilyFromName(kFontChoices[i].family, NULL, &probe) != 0) {
            continue; /* 这台机器上没这个字体，试下一个 */
        }
        if (probe != NULL) {
            family = probe; /* 就它了，一直留着 */
            break;
        }
    }
    return family;
}

GpFont *render_create_font(float pixel_size) {
    GpFontFamily *family = render_family();
    GpFont *font = NULL;

    if (family == NULL || pixel_size <= 0.0f) return NULL;
    if (GdipCreateFont(family, pixel_size, GP_FONT_STYLE_BOLD, GP_UNIT_PIXEL, &font) != 0) {
        return NULL;
    }
    return font;
}

void render_delete_font(GpFont *font) {
    if (font != NULL) GdipDeleteFont(font);
}

void render_text(GpGraphics *graphics, const wchar_t *text, GpFont *font, float rect_x,
                 float rect_y, unsigned int color, unsigned int outline_color, int outline) {
    GpStringFormat *format = NULL;
    GpBrush *fill = NULL;
    GpBrush *edge = NULL;
    GpRectF rect;
    int dx;
    int dy;

    if (graphics == NULL || font == NULL || text == NULL || text[0] == L'\0') return;
    if (GdipCreateStringFormat(GP_FORMAT_NO_WRAP, 0, &format) != 0) return;
    GdipSetStringFormatAlign(format, GP_ALIGN_CENTER);
    GdipSetStringFormatLineAlign(format, GP_ALIGN_CENTER);

    /* 先把描边铺满八个方向，再把字压在上面 —— 和隐私屏幕那只钟同一个做法
     * （那边是 ±2 像素的八个偏移，这里半径按字号缩放）。
     * 同一处地方被叠上八次，抗锯齿的边就叠成了实心描边，而最外圈仍是 alpha 渐变，
     * 于是压在照片和浅色壁纸上都读得清，边缘又不显毛刺。 */
    if (outline > 0 && GdipCreateSolidFill(outline_color, &edge) == 0) {
        for (dy = -outline; dy <= outline; dy += outline) {
            for (dx = -outline; dx <= outline; dx += outline) {
                if (dx == 0 && dy == 0) continue;
                rect.X = rect_x + (float)dx;
                rect.Y = rect_y + (float)dy;
                rect.Width = K_TEXT_RECT_SIZE;
                rect.Height = K_TEXT_RECT_SIZE;
                GdipDrawString(graphics, text, -1, font, &rect, format, edge);
            }
        }
        GdipDeleteBrush(edge);
    }

    if (GdipCreateSolidFill(color, &fill) == 0) {
        rect.X = rect_x;
        rect.Y = rect_y;
        rect.Width = K_TEXT_RECT_SIZE;
        rect.Height = K_TEXT_RECT_SIZE;
        GdipDrawString(graphics, text, -1, font, &rect, format, fill);
        GdipDeleteBrush(fill);
    }

    GdipDeleteStringFormat(format);
}

int render_ink_bounds(const BYTE *bits, int width, int height, RECT *bounds) {
    int left = width;
    int top = height;
    int right = -1;
    int bottom = -1;
    int x;
    int y;

    if (bits == NULL || bounds == NULL || width <= 0 || height <= 0) return 0;

    for (y = 0; y < height; ++y) {
        const BYTE *row = bits + (size_t)y * (size_t)width * 4u;
        for (x = 0; x < width; ++x) {
            if (row[x * 4 + 3] < K_INK_ALPHA_MIN) continue; /* 第四个字节是 alpha */
            if (x < left) left = x;
            if (x > right) right = x;
            if (y < top) top = y;
            if (y > bottom) bottom = y;
        }
    }

    if (right < left || bottom < top) return 0; /* 一个像素都没画上 */

    bounds->left = left;
    bounds->top = top;
    bounds->right = right;
    bounds->bottom = bottom;
    return 1;
}
