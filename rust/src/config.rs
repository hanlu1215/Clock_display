//! ===========================================================================
//!  桌面时钟 —— 配置区（对应 C 版 config.h / config.c）
//!
//!  文字样式参考 win_agent-notify-C 隐私屏幕里那只钟：
//!      Consolas 粗体、纯白、背后一层黑色描边（压在照片/浅色壁纸上也读得清）、
//!      24 小时制 "%H:%M:%S"。等宽字体保证秒数跳动时整串文字一个像素都不抖。
//!  唯一的不同是：这里背景全透明，屏幕上只剩文字。
//! ========================================================================

/// ---- 字体 ----
///
/// 从上往下挑第一个能用的。前三个都是等宽字体：数字和冒号的步进宽度完全一样，
/// 秒数 0→1→2 地跳，整串文字也不会左右晃 —— 隐私屏幕那只钟就是靠 Consolas
/// 等宽做到的，这里沿用。后两个只是万一系统里一个等宽字体都没有时的兜底
/// （Segoe UI / 微软雅黑的数字也是等宽数字，效果一样）。
///
/// 最后的 Comic Sans MS 是手写卡通风的「花活」，只在菜单里手动选，
/// 自动探测永远轮不到它（排在 Segoe UI 后面）。注意它**不等宽**，
/// 秒数跳动时整串字会重新居中、有点晃 —— 要的就是这个味儿。
/// 新字体只能往列表末尾加：ini 里存的是下标，插中间会让老存档错位。
pub const FONT_CHOICES: [&str; 6] = [
    "Consolas",          // 隐私屏幕那只钟用的就是它
    "Lucida Console",    // 等宽，老系统上也有
    "Courier New",       // 等宽，最后的等宽兜底
    "Microsoft YaHei UI",
    "Segoe UI",
    "Comic Sans MS",     // 手写卡通风，Windows 自带
];

/// 「字体」菜单里的特殊项：内置绘制的七段数码（电子表/LCD 风格）。
/// 不是真字体 —— Windows 没有自带七段码字体，引用外部字体名没装就落空，
/// 所以直接用 GDI+ 多边形画（见 render.rs 的 render_segments），零字体文件。
/// 下标 = 真字体列表的长度，刚好接在菜单里所有真字体后面。
pub const SEGMENT_FONT_INDEX: i32 = FONT_CHOICES.len() as i32;
pub const SEGMENT_FONT_NAME: &str = "七段数码（内置）";

/// ---- 字号档位 ----
///
/// 逻辑像素（96 DPI 下的像素），实际渲染时按系统 DPI 等比放大。
/// 右键菜单「字号」里就是这几档，鼠标滚轮可以逐档加减。
///
/// 96 是默认档：1080p 下占屏高约 1/11，摆在桌面角落不挡事又足够看清。
/// 隐私屏幕那只钟是屏高的 16%（1080p 上 172px）—— 那是全屏遮挡用的尺寸，
/// 常驻桌面太占地方，所以这里默认小一档，想要那种效果选 172 即可。
pub const SIZE_STEPS: [i32; 10] = [24, 32, 48, 64, 96, 128, 172, 216, 288, 384];
pub const DEFAULT_SIZE_INDEX: usize = 4; // 96 px

/// ---- 颜色档位（0xAARRGGBB）----
///
/// 第一项是白色 —— 和隐私屏幕那只钟一致。其余几个是为了压在浅色壁纸上时能换色。
pub struct ColorChoice {
    pub name: &'static str,
    pub argb: u32,
}

pub const COLOR_CHOICES: [ColorChoice; 10] = [
    ColorChoice { name: "白色", argb: 0xFFFFFFFF },
    ColorChoice { name: "金色", argb: 0xFFFFD88A },
    ColorChoice { name: "橙色", argb: 0xFFFF9F45 },
    ColorChoice { name: "粉色", argb: 0xFFFF7FB0 },
    ColorChoice { name: "红色", argb: 0xFFFF5F5F },
    ColorChoice { name: "青色", argb: 0xFF43D6C8 },
    ColorChoice { name: "绿色", argb: 0xFF4EDE9A },
    ColorChoice { name: "蓝色", argb: 0xFF5AA9FF },
    ColorChoice { name: "紫色", argb: 0xFFB48CFF },
    ColorChoice { name: "灰色", argb: 0xFFC9D1DC },
];

/// ---- 默认值 ----
pub const DEFAULT_COLOR: u32 = 0xFFFFFFFF; // 白
pub const DEFAULT_TOPMOST: bool = true;
pub const DEFAULT_OUTLINE: bool = true;

/// 字体下标的默认值：-1 = 自动（按 FONT_CHOICES 顺序挑第一个装了的，老行为）。
/// 用户在右键菜单「字体」里选过之后就是具体的下标；选中的字体在别的机器上
/// 不存在时会回落到自动。
pub const DEFAULT_FONT_INDEX: i32 = -1;

/// ---- 排版 ----
///
/// 描边半径按字号等比缩放：隐私屏幕那只钟是 172px 字号配 ±2px 描边，
/// 这里照这个比例走（字号 ÷ 86），再小的字号也至少留 1px。
pub const OUTLINE_DIVISOR: f32 = 86.0;

/// 墨迹四周的透明留白：描边半径 + 一点呼吸空间。
/// 留白不挡鼠标 —— 分层窗口里 alpha 为 0 的地方本来就让鼠标穿过去。
pub const PAD_MIN: i32 = 3;

/// 墨迹判定阈值：alpha 小于这个值算「什么都没画」。
/// 取 16 是为了滤掉抗锯齿最外面那圈几乎看不见的毛边。
pub const INK_ALPHA_MIN: u8 = 16;

/// ---- 刷新 ----
///
/// 250ms 轮询一次，只在「秒」真的跳了才重画。比死等 1000ms 稳：
/// 系统一忙，定时器就会被拖后，靠 1 秒一跳会越走越慢。
pub const TICK_MS: u32 = 250;

/// ---- 位置 ----
///
/// 第一次运行（还没有存档位置）时：水平居中，纵向离工作区顶部这个比例。
pub const START_TOP_RATIO: f64 = 0.15;
pub const POS_UNSET: i32 = i32::MIN;

/// ---- 拖动 ----
///
/// 鼠标移动超过这么多像素才算「拖动」，否则只当成点了一下。
pub const DRAG_THRESHOLD: i32 = 3;

/// ---- 存档 ----
///
/// 跟 exe 放一起：绿色软件，整个文件夹拷走，位置/颜色/字号也跟着走。
/// （放在 Program Files 这类没有写权限的地方时会存不下来，程序照常跑。）
pub const INI_NAME: &str = "clock.ini";
pub const INI_SECTION: &str = "clock";
