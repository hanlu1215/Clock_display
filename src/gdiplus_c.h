#pragma once

/* ===========================================================================
 *  GDI+ 扁平（C）API 的最小声明。
 *
 *  为什么不直接 #include <gdiplus.h>：MinGW 里那份头文件是给 C++ 用的 ——
 *  GpStatus / REAL / GpBitmap 这些类型定义在只认 C++ 的 gdiplustypes.h 里，
 *  纯 C 编不过。而 gdiplus.dll 导出的本来就是这一套 GdipXxx 扁平函数
 *  （GDI+ 的 C++ 类不过是在它们外面包了一层），MinGW 的 libgdiplus.a 里
 *  这些都实实在在地导出了。所以这里照着 SDK 的 gdiplusflat.h 把用得到的
 *  那几十个声明一遍，句柄一律用不透明指针，枚举常量照抄取值（都注了出处），
 *  于是纯 C 也能用上 GDI+ 的抗锯齿文字。
 *
 *  取值的出处：
 *      gdiplusenums.h        —— 单位、字体样式、对齐、抗锯齿、FlushIntention
 *      gdipluspixelformats.h —— 像素格式
 *      gdiplusinit.h         —— GdiplusStartupInput 的字段顺序
 *  这些都是 GDI+ 问世以来没动过的 ABI 常量，可以放心硬编码。
 * ======================================================================== */

#include <windows.h>

/* 调用约定照抄 gdiplustypes.h：x86 上是 __stdcall，ARM 上是默认 */
#if defined(_ARM_)
#define GDIP_API
#else
#define GDIP_API __stdcall
#endif

typedef int GpStatus;
typedef float REAL;
typedef int PixelFormat;
typedef unsigned int ARGB;

/* 不透明句柄：程序只拿着指针传来传去，结构体长什么样无关紧要。
 * （GdipGetImageGraphicsContext / GdipDisposeImage 在头文件里收的是 GpImage*，
 *  这里统一写成 GpBitmap*：都是不透明指针，ABI 上完全一致，省得来回强转。） */
typedef struct GpGraphics GpGraphics;
typedef struct GpBitmap GpBitmap;
typedef struct GpFont GpFont;
typedef struct GpFontFamily GpFontFamily;
typedef struct GpBrush GpBrush;
typedef struct GpStringFormat GpStringFormat;

typedef struct GpRectF {
    REAL X;
    REAL Y;
    REAL Width;
    REAL Height;
} GpRectF;

/* 字段顺序 = gdiplusinit.h 里的 GdiplusStartupInput */
typedef struct GdiplusStartupInput {
    UINT32 GdiplusVersion;
    void *DebugEventCallback;
    BOOL SuppressBackgroundThread;
    BOOL SuppressExternalCodecs;
} GdiplusStartupInput;

/* ---- 枚举取值 ---- */
#define GP_UNIT_PIXEL 2             /* UnitPixel */
#define GP_FONT_STYLE_BOLD 1        /* FontStyleBold */
#define GP_ALIGN_NEAR 0             /* StringAlignmentNear */
#define GP_ALIGN_CENTER 1           /* StringAlignmentCenter */
#define GP_TEXT_HINT_ANTIALIAS 4    /* TextRenderingHintAntiAlias */
#define GP_SMOOTHING_ANTIALIAS 4    /* SmoothingModeAntiAlias */
#define GP_PIXEL_OFFSET_HALF 4      /* PixelOffsetModeHalf */
#define GP_FLUSH_SYNC 1             /* FlushIntentionSync */
#define GP_FORMAT_NO_WRAP 0x00001000 /* StringFormatFlagsNoWrap */
/* PixelFormat32bppPARGB = 11 | (32<<8) | Alpha | PAlpha | GDI = 0x000E200B */
#define GP_PIXEL_FORMAT_32BPP_PARGB 0x000E200B

/* ---- 启动 / 关闭 ---- */
GpStatus GDIP_API GdiplusStartup(ULONG_PTR *token, const GdiplusStartupInput *input,
                                 void *output);
void GDIP_API GdiplusShutdown(ULONG_PTR token);

/* ---- 位图与画布 ---- */
GpStatus GDIP_API GdipCreateBitmapFromScan0(INT width, INT height, INT stride,
                                            PixelFormat format, BYTE *scan0,
                                            GpBitmap **bitmap);
GpStatus GDIP_API GdipGetImageGraphicsContext(GpBitmap *image, GpGraphics **graphics);
GpStatus GDIP_API GdipDisposeImage(GpBitmap *image);
GpStatus GDIP_API GdipDeleteGraphics(GpGraphics *graphics);
GpStatus GDIP_API GdipGraphicsClear(GpGraphics *graphics, ARGB color);
GpStatus GDIP_API GdipFlush(GpGraphics *graphics, INT intention);

/* ---- 画布状态 ---- */
GpStatus GDIP_API GdipSetSmoothingMode(GpGraphics *graphics, int mode);
GpStatus GDIP_API GdipSetPixelOffsetMode(GpGraphics *graphics, int mode);
GpStatus GDIP_API GdipSetTextRenderingHint(GpGraphics *graphics, int hint);
GpStatus GDIP_API GdipSetTextContrast(GpGraphics *graphics, UINT contrast);

/* ---- 字体 ---- */
/* 第二个参数是 GpFontCollection*：传 NULL 表示查系统字体集，
 * 名字找不到时返回非 0 —— 正好拿来一个个试字体表里的候选。 */
GpStatus GDIP_API GdipCreateFontFamilyFromName(const WCHAR *name, void *fontCollection,
                                               GpFontFamily **fontFamily);
GpStatus GDIP_API GdipDeleteFontFamily(GpFontFamily *fontFamily);
GpStatus GDIP_API GdipCreateFont(const GpFontFamily *fontFamily, REAL emSize, INT style,
                                 int unit, GpFont **font);
GpStatus GDIP_API GdipDeleteFont(GpFont *font);

/* ---- 画刷 ---- */
GpStatus GDIP_API GdipCreateSolidFill(ARGB color, GpBrush **brush);
GpStatus GDIP_API GdipDeleteBrush(GpBrush *brush);

/* ---- 文字排版与绘制 ---- */
GpStatus GDIP_API GdipCreateStringFormat(INT formatAttributes, LANGID language,
                                         GpStringFormat **format);
GpStatus GDIP_API GdipDeleteStringFormat(GpStringFormat *format);
GpStatus GDIP_API GdipSetStringFormatAlign(GpStringFormat *format, int align);
GpStatus GDIP_API GdipSetStringFormatLineAlign(GpStringFormat *format, int align);
GpStatus GDIP_API GdipSetStringFormatFlags(GpStringFormat *format, INT flags);
GpStatus GDIP_API GdipDrawString(GpGraphics *graphics, const WCHAR *string, INT length,
                                 const GpFont *font, const GpRectF *layoutRect,
                                 const GpStringFormat *stringFormat, const GpBrush *brush);
