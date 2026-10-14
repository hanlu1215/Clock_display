# 桌面时钟 —— Rust 版

根目录 C 版的 Rust 移植：同样的外观、同样的交互、`clock.ini` 存档格式兼容。
实现说明见根目录 [README.md](../README.md)，那边的「几个值得说明的实现点」
全部适用于本版。

与 C 版的一处差异：右键菜单多了「**字体**」子菜单（C 版暂时只能在
`config.c` 里改字体表）。列出 `FONT_CHOICES` 里的候选字体，默认是「自动」
（从上往下挑第一个装了的，即 C 版的老行为）；选中的字体存进 ini 的 `font`
键（-1 = 自动），在别的机器上缺失时回落到自动。菜单里打勾的是**实际生效**
的字体。C 版读到这个键会忽略，不影响兼容。

字体菜单里有两个特别的：

- **Comic Sans MS**：手写卡通风，Windows 自带。注意它**不等宽**，秒数跳动时
  整串字会重新居中、有点晃 —— 要的就是这个味儿。
- **七段数码（内置）**：电子表 / LCD 风格。Windows 没有自带七段码字体，
  所以是程序自己用 GDI+ 多边形画的（`render.rs` 的 `render_segments`），
  零字体文件、零新依赖，排版/描边/配色与其他字体走同一条管线。

## 构建

Windows + Rust（MSVC 工具链）：

```bash
cargo build --release
```

产物在 `target/release/clock_display.exe`。

**依赖**：只有 Windows 自带的 DLL（`gdiplus` / `user32` / `gdi32` / `comdlg32` /
`kernel32` / `ntdll`）。CRT 已用 `+crt-static` 静态链入（见
[.cargo/config.toml](.cargo/config.toml)），没有 `vcruntime140.dll` 依赖，
exe 拷到别的 Windows 10/11 上直接双击就能跑。

清单（comctl32 v6 现代菜单 + DPI 感知）、图标、版本信息由
[build.rs](build.rs) 用 `embed-resource` 编进 exe；清单和 ico 直接复用
C 版 `../res/` 目录里的文件。

## 代码结构

模块和 C 版一一对应，注释也对齐：

```
src/
  main.rs      入口：GDI+ 起停、命令行、帮助        ← main.c
  clock.rs     时钟本体：窗口、消息、拖动、排版、刷新 ← clock.c
  menu.rs      右键菜单：颜色（含自定义）/ 字号 / 描边 / 置顶 / 关闭 ← menu.c
  render.rs    字体与文字绘制（带描边）              ← render.c
  surface.rs   32bpp 预乘 alpha 内存画布 + UpdateLayeredWindow ← surface.c
  settings.rs  clock.ini 读写                       ← settings.c
  config.rs    配置区：字体表、字号档位、颜色档位、各默认值 ← config.h/.c
  util.rs      小工具：DPI、取时间、亮度判断         ← util.c
```

和 C 版的几处对应关系说明：

- Win32 / GDI+ 调用走官方 `windows` crate（GDI+ 扁平 API 在
  `Win32_Graphics_GdiPlus` feature 里），C 版 `gdiplus_c.h` 手写的那些声明
  就不需要了。
- 窗口过程仍是「`Box::into_raw` + `GWLP_USERDATA`」的老模式，unsafe 集中在
  `clock.rs` 的 `clock_window_proc`。
- 字体 family 和 C 版一样**故意泄漏**（`OnceLock` 缓存，永不释放）：
  GdiplusShutdown 之后再析构 GDI+ 对象必崩。
- 其余 GDI+ 对象（font / brush / graphics / bitmap）由带 `Drop` 的薄包装管理，
  `main` 里用作用域保证它们在 `GdiplusShutdown` 之前析构干净（同 C 版）。

## 命令行

```
clock_display.exe [选项]

  -r, --reset    恢复默认位置、字号和颜色
  -h, --help     显示帮助
```
