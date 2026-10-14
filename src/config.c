#include "config.h"

const FontChoice kFontChoices[] = {
    {L"Consolas"},          /* 隐私屏幕那只钟用的就是它 */
    {L"Lucida Console"},    /* 等宽，老系统上也有 */
    {L"Courier New"},       /* 等宽，最后的等宽兜底 */
    {L"Microsoft YaHei UI"},
    {L"Segoe UI"},
};
const int kFontChoiceCount = (int)(sizeof(kFontChoices) / sizeof(kFontChoices[0]));

/* 96 是默认档：1080p 下占屏高约 1/11，摆在桌面角落不挡事又足够看清。
 * 隐私屏幕那只钟是屏高的 16%（1080p 上 172px）—— 那是全屏遮挡用的尺寸，
 * 常驻桌面太占地方，所以这里默认小一档，想要那种效果选 172 即可。 */
const int kSizeSteps[] = {24, 32, 48, 64, 96, 128, 172, 216, 288, 384};
const int kSizeStepCount = (int)(sizeof(kSizeSteps) / sizeof(kSizeSteps[0]));

const ColorChoice kColorChoices[] = {
    {L"白色", 0xFFFFFFFFu},
    {L"金色", 0xFFFFD88Au},
    {L"橙色", 0xFFFF9F45u},
    {L"粉色", 0xFFFF7FB0u},
    {L"红色", 0xFFFF5F5Fu},
    {L"青色", 0xFF43D6C8u},
    {L"绿色", 0xFF4EDE9Au},
    {L"蓝色", 0xFF5AA9FFu},
    {L"紫色", 0xFFB48CFFu},
    {L"灰色", 0xFFC9D1DCu},
};
const int kColorChoiceCount = (int)(sizeof(kColorChoices) / sizeof(kColorChoices[0]));
