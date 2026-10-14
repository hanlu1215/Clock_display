"""看一眼 exe 的 PE 头，守住这个项目的两条底线。

    1) 子系统必须是 Windows GUI —— 否则双击会先弹一个命令提示符窗口
    2) 依赖的 DLL 必须全是 Windows 自带的 —— 否则拷到别的电脑上起不来

只用标准库（PE 头是按结构体偏移直接读的），所以 CI 上不用装任何东西。
本地也能跑：

    python tools/check_pe.py rust/target/release/clock_display.exe
    python tools/check_pe.py build/bin/clock_display.exe

不通过就 exit 1，顺带把是哪一条没过、以及该去改哪里打出来。
"""

import struct
import sys


def force_utf8_output():
    """把 stdout/stderr 掰成 UTF-8。

    Windows 上 Python 的 stdout 默认按系统代码页编码 —— CI 的英文镜像上是
    cp1252，打中文直接 UnicodeEncodeError 崩掉，报的还是一条跟检查内容毫无
    关系的编码错误（踩过）。GitHub Actions 的日志本来就按 UTF-8 解，
    VS Code / Git Bash 终端也是 UTF-8，统一成 UTF-8 最省事。
    """
    for stream in (sys.stdout, sys.stderr):
        try:
            stream.reconfigure(encoding="utf-8", errors="replace")
        except (AttributeError, ValueError):
            pass  # 3.7 以前没有 reconfigure，或者流不支持：那就随它去


force_utf8_output()

# 允许出现的依赖：Windows 自带。
# 少了 libgcc_s / libwinpthread（MinGW 运行时）和 vcruntime140 / msvcp140
# （MSVC 运行时）—— 那几个不是系统组件，缺了就得装运行库，"拷过去双击就能跑"
# 这条底线就破了，所以它们必须让检查红掉。
ALLOWED_EXACT = {
    "kernel32.dll",
    "user32.dll",
    "gdi32.dll",
    "gdiplus.dll",
    "comdlg32.dll",
    "shell32.dll",
    "advapi32.dll",
    "ole32.dll",
    "oleaut32.dll",
    "uuid.dll",
    "ntdll.dll",
    "ucrtbase.dll",  # Windows 10 起是系统组件
    "msvcrt.dll",
}
ALLOWED_PREFIX = ("api-ms-win-",)  # API set，Win10 起全部自带

# 常见的"不是系统组件"的依赖，单独给一句提示，省得对着 DLL 名发愣
HINTS = {
    "libgcc_s": "MinGW 的 libgcc 没静态链上，检查 CMakeLists.txt 里的 -static / -static-libgcc",
    "libwinpthread": "MinGW 的 pthread 没静态链上，同上",
    "libstdc++": "MinGW 的 libstdc++ 没静态链上，同上",
    "vcruntime": "MSVC 的 CRT 没静态链上，检查 rust/.cargo/config.toml 里的 -C target-feature=+crt-static",
    "msvcp": "MSVC 的 C++ 运行库没静态链上，同上",
}

SUBSYSTEM_NAMES = {2: "Windows GUI", 3: "Windows CUI（控制台）"}
IMAGE_SUBSYSTEM_WINDOWS_GUI = 2


class PeError(Exception):
    pass


def read_pe(path):
    """返回 (subsystem, [依赖的 DLL 名])。"""
    data = open(path, "rb").read()

    if data[:2] != b"MZ":
        raise PeError("不是 PE 文件（没有 MZ 头）")
    pe = struct.unpack_from("<I", data, 0x3C)[0]
    if data[pe : pe + 4] != b"PE\0\0":
        raise PeError("不是 PE 文件（没有 PE 签名）")

    num_sections = struct.unpack_from("<H", data, pe + 6)[0]
    opt_size = struct.unpack_from("<H", data, pe + 20)[0]
    opt = pe + 24

    magic = struct.unpack_from("<H", data, opt)[0]
    if magic == 0x10B:
        data_dir = opt + 96
    elif magic == 0x20B:
        data_dir = opt + 112
    else:
        raise PeError("看不懂的 optional header magic 0x%X" % magic)

    subsystem = struct.unpack_from("<H", data, opt + 68)[0]

    # 节表：RVA -> 文件偏移要用它换算
    sections = []
    sec = opt + opt_size
    for i in range(num_sections):
        base = sec + i * 40
        vsize, vaddr, rawsize, rawptr = struct.unpack_from("<IIII", data, base + 8)
        sections.append((vaddr, max(vsize, rawsize), rawptr))

    def rva_to_offset(rva):
        for vaddr, size, rawptr in sections:
            if vaddr <= rva < vaddr + size:
                return rawptr + (rva - vaddr)
        return None

    # 数据目录第 1 项 = 导入表
    import_rva = struct.unpack_from("<I", data, data_dir + 8)[0]
    if import_rva == 0:
        return subsystem, []

    names = []
    off = rva_to_offset(import_rva)
    if off is None:
        raise PeError("导入表的 RVA 落不到任何节里")

    # 导入描述符 20 字节一条，全 0 那条收尾
    while True:
        entry = data[off : off + 20]
        if len(entry) < 20:
            break
        _orig, _stamp, _chain, name_rva, _first = struct.unpack("<IIIII", entry)
        if name_rva == 0:
            break
        name_off = rva_to_offset(name_rva)
        if name_off is None:
            break
        end = data.index(b"\0", name_off)
        names.append(data[name_off:end].decode("ascii", "replace"))
        off += 20

    return subsystem, names


def allowed(name):
    low = name.lower()
    if low in ALLOWED_EXACT:
        return True
    return any(low.startswith(p) for p in ALLOWED_PREFIX)


def check(path):
    print("== %s" % path)
    try:
        subsystem, imports = read_pe(path)
    except (PeError, OSError) as exc:
        print("  [失败] 读不了：%s" % exc)
        return False

    print("  子系统: %s" % SUBSYSTEM_NAMES.get(subsystem, "未知(0x%X)" % subsystem))
    shown = sorted({n for n in imports if n})
    for name in shown:
        print("    依赖: %s" % name)

    ok = True

    if subsystem != IMAGE_SUBSYSTEM_WINDOWS_GUI:
        print("  [失败] 不是窗口子系统 —— 双击会先弹一个命令提示符窗口")
        ok = False

    bad = [n for n in shown if not allowed(n)]
    if bad:
        print("  [失败] 依赖了不在白名单里的 DLL：")
        for name in bad:
            low = name.lower()
            hint = next((v for k, v in HINTS.items() if k in low), None)
            if hint:
                print("     - %s —— %s" % (name, hint))
            else:
                print("     - %s —— 确认它是 Windows 自带的系统 DLL 就加进 ALLOWED_EXACT，"
                      "否则得想办法静态链进去" % name)
        ok = False

    if ok:
        print("  [通过] 窗口子系统 + 依赖全是 Windows 自带的 DLL")
    return ok


def main(argv):
    if len(argv) < 2:
        print(__doc__)
        return 2
    ok = True
    for path in argv[1:]:
        if not check(path):  # 不短路：有几个文件就查几个，一次把问题都报出来
            ok = False
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
