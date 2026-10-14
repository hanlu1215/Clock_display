#pragma once

#include <windows.h>

/* ===========================================================================
 *  桌面时钟 —— 配置区
 *
 *  文字样式参考 win_agent-notify-C 隐私屏幕里那只钟：
 *      Consolas 粗体、纯白、背后一层黑色描边（压在照片/浅色壁纸上也读得清）、
 *      24 小时制 "%H:%M:%S"。等宽字体保证秒数跳动时整串文字一个像素都不抖。
 *  唯一的不同是：这里背景全透明，屏幕上只剩文字。
 * ======================================================================== */

/* ---- 字体 ----
 *
 * 从上往下挑第一个能用的。前三个都是等宽字体：数字和冒号的步进宽度完全一样，
 * 秒数 0→1→2 地跳，整串文字也不会左右晃 —— 隐私屏幕那只钟就是靠 Consolas
 * 等宽做到的，这里沿用。后两个只是万一系统里一个等宽字体都没有时的兜底
 * （Segoe UI / 微软雅黑的数字也是等宽数字，效果一样）。 */
typedef struct {
    const wchar_t *family;
} FontChoice;

extern const FontChoice kFontChoices[];
extern const int kFontChoiceCount;

/* ---- 字号档位 ----
 *
 * 逻辑像素（96 DPI 下的像素），实际渲染时按系统 DPI 等比放大。
 * 右键菜单「字号」里就是这几档，鼠标滚轮可以逐档加减。 */
extern const int kSizeSteps[];
extern const int kSizeStepCount;
#define K_DEFAULT_SIZE_INDEX 4 /* 96 px */

/* ---- 颜色档位（0xAARRGGBB）----
 *
 * 第一项是白色 —— 和隐私屏幕那只钟一致。其余几个是为了压在浅色壁纸上时能换色。 */
typedef struct {
    const wchar_t *name;
    unsigned int argb;
} ColorChoice;

extern const ColorChoice kColorChoices[];
extern const int kColorChoiceCount;

/* ---- 默认值 ---- */
#define K_DEFAULT_COLOR 0xFFFFFFFFu /* 白 */
#define K_DEFAULT_TOPMOST 1
#define K_DEFAULT_OUTLINE 1

/* ---- 排版 ----
 *
 * 描边半径按字号等比缩放：隐私屏幕那只钟是 172px 字号配 ±2px 描边，
 * 这里照这个比例走（字号 ÷ 86），再小的字号也至少留 1px。 */
#define K_OUTLINE_DIVISOR 86

/* 墨迹四周的透明留白：描边半径 + 一点呼吸空间。
 * 留白不挡鼠标 —— 分层窗口里 alpha 为 0 的地方本来就让鼠标穿过去。 */
#define K_PAD_MIN 3

/* 墨迹判定阈值：alpha 小于这个值算「什么都没画」。
 * 取 16 是为了滤掉抗锯齿最外面那圈几乎看不见的毛边。 */
#define K_INK_ALPHA_MIN 16

/* ---- 刷新 ----
 *
 * 250ms 轮询一次，只在「秒」真的跳了才重画。比死等 1000ms 稳：
 * 系统一忙，定时器就会被拖后，靠 1 秒一跳会越走越慢。 */
#define K_TICK_MS 250

/* ---- 位置 ----
 *
 * 第一次运行（还没有存档位置）时：水平居中，纵向离工作区顶部这个比例。 */
#define K_START_TOP_RATIO 0.15
#define K_POS_UNSET INT_MIN

/* ---- 拖动 ----
 *
 * 鼠标移动超过这么多像素才算「拖动」，否则只当成点了一下。 */
#define K_DRAG_THRESHOLD 3

/* ---- 存档 ----
 *
 * 跟 exe 放一起：绿色软件，整个文件夹拷走，位置/颜色/字号也跟着走。
 * （放在 Program Files 这类没有写权限的地方时会存不下来，程序照常跑。） */
#define K_INI_NAME L"clock.ini"
#define K_INI_SECTION L"clock"
