# 桌面时钟（Clock Display）

屏幕上只剩一行时间，背景**全透明** —— 没有窗口边框、没有底色、不进任务栏。

文字样式参考 `AGENT_MONITORING/win_agent-notify-C` 隐私屏幕里那只钟：
**Consolas 粗体、纯白、背后一层黑色描边、24 小时制 `%H:%M:%S`**。
区别只在于那只钟压在整屏黑底上，这只是把背景整个去掉，只剩文字浮在桌面上。

> 这份 README 讲的是**仓库根目录的 C 版**（Win32 + GDI+，纯 C）。
> Rust 版在 [rust/](rust/) 目录，功能 1:1 移植，用法和设置文件都一样，
> 见 [rust/README.md](rust/README.md)。两版各有独立的构建和发版 workflow。

## 操作

| 操作 | 效果 |
| --- | --- |
| 左键按住文字拖动 | 移动时钟（**只有文字本身点得中**，透明的地方鼠标会穿过去，点不到） |
| 鼠标滚轮 | 字号放大 / 缩小，以文字中心为锚点，不会跑偏 |
| 右键 | 菜单：颜色 / 字号 / 描边 / 置顶 / 关闭 |
| Esc | 关闭（时钟要先被点过一下才收得到键盘） |

颜色除了十个预设，还能选「自定义…」，弹的是系统调色板。

设置**自动存在 exe 同目录的 `clock.ini`**：位置、字号、颜色、描边、置顶。
绿色软件，整个文件夹拷到别的机器上，这些设置一起跟着走。

## 命令行

```
clock_display.exe [选项]

  -r, --reset    恢复默认位置、字号和颜色
                 （时钟被拖到屏幕外找不回来时用这个）
  -h, --help     显示帮助
```

不带参数直接双击就行 —— 它是窗口子系统程序，**不会弹命令提示符窗口**。

## 构建

Windows + MSYS2（UCRT64）+ CMake + Ninja：

```bash
cmake -S . -B build -G Ninja
cmake --build build
```

产物在 `build/bin/clock_display.exe`。

要清一遍重来：

```bash
rm -rf build && cmake -S . -B build -G Ninja && cmake --build build
```

**依赖**：只有 Windows 自带的 DLL（`user32` / `gdi32` / `gdiplus` / `comdlg32` /
`shell32` / `kernel32`）。MinGW 那套运行时（`libgcc`、`libwinpthread`）已经用
`-static` 静态链进去了，exe 拷到别的 Windows 10/11 上直接双击就能跑，不需要装
MSYS2 或任何运行库。

> UCRT64 工具链编译出来的程序依赖「通用 C 运行库」（UCRT）。Windows 10/11 自带；
> Windows 7/8.1 需要装一次 KB2999226。想连这个都不要，可以把工具链换成 MSYS2 的
> `mingw64`（MSVCRT 那套，链的是所有 Windows 都有的 `msvcrt.dll`），
> `CMakeLists.txt` 不用改。

## 发版（GitHub Actions）

C 版和 Rust 版各一个 workflow，**互相独立、各点各的、各发各的 release**。
两个都**只在手动点击时跑**，不挂 `push` —— 平时提交什么都不做，要发版了再点一下。

| | C 版 | Rust 版 |
| --- | --- | --- |
| workflow | [build-release.yml](.github/workflows/build-release.yml) | [build-release-rust.yml](.github/workflows/build-release-rust.yml) |
| 名字 | 构建并发布 Windows 版（C） | 构建并发布 Windows 版（Rust） |
| 工具链 | MSYS2 UCRT64 + CMake + Ninja | rustup / MSVC |
| 产物 | `clock_display-c-<tag>-win64.zip` | `clock_display-rust-<tag>-win64.zip` |
| 默认 tag | `v1.0.0` | `rust-v1.0.0` |

仓库页 → **Actions** → 左边选一个 → 右上 **Run workflow**，参数两个 workflow 一样：

| 参数 | 说明 |
| --- | --- |
| `tag` | 发布用的 tag，默认见上表；已存在就更新那个 release |
| `prerelease` | 标成预发布 |
| `draft` | 存成草稿，先不公开 |

点下去之后它会：装工具链 → 构建 → **自检** → 打包（exe + README）→ 上传成构建产物 →
创建 / 更新对应的 GitHub Release 并把 zip 挂上去。

> 默认 tag 加了 `rust-` 前缀，是为了让两个 release 分开 —— 两个 exe 都叫
> `clock_display.exe`，挤在同一个 release 里说明文字会互相覆盖。想合成一个
> release 的话，把 Rust 那个的默认 tag 改成 `v1.0.0`，再把两边的 `body` 换掉、
> 加上 `append_body: true` 就行。

### 那道自检是干嘛的

两个 workflow 都会在打包前卡一道检查，守着这个项目的两条底线：

1. **子系统必须是 Windows GUI** —— 否则双击会先弹一个命令提示符窗口
2. **依赖的 DLL 必须全是 Windows 自带的** —— 否则拷到别的电脑上起不来

C 版用 MSYS2 里现成的 `objdump`（它本来就要装 MSYS2 编译）；
Rust 版用 [tools/check_pe.py](tools/check_pe.py) —— 只读 PE 头、只用标准库，
runner 上现成的 python 就够，不用为了看一眼依赖再拉一整套 MSYS2 进来。
本地也能跑：

```bash
python tools/check_pe.py rust/target/release/clock_display.exe
python tools/check_pe.py build/bin/clock_display.exe
```

这道检查不是摆设，它真抓到过东西：Rust 版如果在仓库根上用
`cargo build --manifest-path rust/Cargo.toml` 构建，cargo 就找不到
`rust/.cargo/config.toml`，`+crt-static` 静默失效，**构建照样成功**，但 exe
会依赖 `VCRUNTIME140.dll`。所以 Rust workflow 里那步构建特意站在 `rust/`
目录里跑，`check_pe.py` 就是那个负责当场把它喊出来的。

> 两个 workflow 第一次跑都要多花两三分钟（C 版要滚一遍 MSYS2 仓库；
> Rust 版要编 `windows` crate）。另外建 release 需要写权限，workflow 里都
> 声明了 `permissions: contents: write`；如果仓库设置里把 Workflow permissions
> 卡成了只读，去 Settings → Actions → General 放开。

## 代码结构

```
src/
  main.c       入口：GDI+ 起停、命令行、帮助
  clock.c      时钟本体：窗口、消息、拖动、排版、刷新
  menu.c       右键菜单：颜色（含自定义）/ 字号 / 描边 / 置顶 / 关闭
  render.c     字体与文字绘制（带描边）
  surface.c    32bpp 预乘 alpha 内存画布 + UpdateLayeredWindow
  settings.c   clock.ini 读写
  config.h/.c  配置区：字体表、字号档位、颜色档位、各默认值
  gdiplus_c.h  GDI+ 扁平（C）API 的最小声明
  util.c       小工具：DPI、取时间、亮度判断
res/
  app.rc.in    资源脚本模板（图标 + 清单 + 版本信息）
  app.manifest 清单：comctl32 v6（现代右键菜单）+ DPI 感知
  clock.ico    exe 图标（7 个尺寸）
tools/
  make_icon.py 重新生成 clock.ico（只用标准库）
  check_pe.py  检查 exe 的子系统与 DLL 依赖（CI 自检用，也可本地跑）
rust/          Rust 版（1:1 移植），见 rust/README.md
.github/workflows/
  build-release.yml       手动触发的构建 + 发版（C 版）
  build-release-rust.yml  手动触发的构建 + 发版（Rust 版）
```

### 几个值得说明的实现点

**背景为什么能真透明。** 用的是 `WS_EX_LAYERED` 窗口 + `UpdateLayeredWindow`，
画布是 32bpp **预乘 alpha** 的 DIB，GDI+ 直接往上面画。逐像素 alpha 混合，
所以抗锯齿的文字边缘过渡得很干净，不像「色键抠图」那样在字边上留一圈杂色。
（这一套和 `deepseek_balance` 那个小方块是同一个路子。）

**「只有文字能点中」是白送的。** 分层窗口在 alpha 为 0 的地方本来就把鼠标消息
放行给下面的窗口，所以不用自己去算命中区域 —— 点着数字能拖，戳到空白处直接
穿到桌面或后面的窗口上。可以拿 `WindowFromPoint` 验证：数字上返回时钟窗口，
数字之间的空隙返回下面的窗口。

**文字为什么不会抖。** 字体首选 Consolas（等宽）：每个数字和冒号的步进宽度
一模一样，秒数从 8 跳到 0 整串字也不会左右晃 —— 隐私屏幕那只钟也是靠这个。
另外量排版用的是替身字符串 `88:88:88`，窗口尺寸是按它定死的，实际时间
只是画进这块固定的框里，所以窗口本身纹丝不动。

**墨迹定位。** 不用 `GdipMeasureString` 的结果当尺寸：它量出来的是「行框」，
上下留着上伸部/下伸部的空档（数字既够不到顶也踩不到底），照着裁出来的窗口
看着就是歪的；而且它量出的框还比字实际占的地方大一圈（Consolas 粗体 96px 下
量 `88:88:88` 是 467×124，真正画上去的墨迹只有 425×64），照它收紧的话 GDI+
居中会飘，整串字往右偏、末尾数字被窗口切掉。

所以改成：真画一次 → 扫 alpha 找出墨迹的真实范围（含描边）→ 拿它定窗口尺寸和
位置。排版矩形固定 4000×4000（远大于任何文字），矩形左上角挪多少墨迹就挪多少，
是线性的，位置完全可算。

**描边。** 和隐私屏幕那只钟同一个做法：黑边铺满八个方向，再把字压在上面。
那边是固定 ±2 像素，这里按字号等比缩放（字号 ÷ 86，96px 配 1px、172px 配 2px，
和那只钟在 1080p 下的比例一致）。颜色跟着字色走：亮字配黑边，暗字配白边，
所以压在任何壁纸上都读得清。不想要可以在菜单里关掉。

**刷新。** 250ms 轮询一次，只在「秒」真的跳了才重画。比死等 1000ms 稳：
系统一忙，定时器就会被拖后，靠 1 秒一跳会越走越慢。

**置顶为什么要定期重申一次。** `WS_EX_TOPMOST` 是一个「层」而不是「第一位」：
普通窗口永远翻不到它上面，但层**内部**的顺序按「谁最近被激活 / 最近插进来」排。
启动时插到层顶之后就不管的话，后面陆续冒出来的置顶窗口（别的浮窗、输入法的
IME 窗口……）会一个个插到前面，时间一长时钟就被挤到层底 —— 看着像被盖住了，
点一下激活它才会重新排上去。所以每隔一阵子 `SetWindowPos(HWND_TOPMOST, ...,
SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE)` 重申一次，把它按回层顶。

重申的时机是**「分」跳了的时候**，不是每秒：每秒都动一次 z 序太吵，会打扰用户
正在进行的操作（拖别的窗口、全屏应用……），一分钟一次足够了 —— 真被盖住了
点一下时钟也会自己回到层顶。注意重申只在「置顶」开着时才做，否则等于把用户的
设置顶回去。

## 图标

`res/clock.ico` 是把 Python 脚本画出来的（`tools/make_icon.py`，只用标准库，
不依赖 Pillow）：深色表盘 + 浅色外圈 + 白色时针分针 + 金色秒针，
包含 16/24/32/48/64/128/256 七个尺寸，每个尺寸都是按覆盖率算的抗锯齿。

想换配色或指针角度，改脚本顶上的常量再跑一次：

```bash
python tools/make_icon.py res/clock.ico
```

重新构建时 `CMakeLists.txt` 会自动带上它；`res/clock.ico` 不存在也能正常编译，
只是 exe 用系统默认图标。
