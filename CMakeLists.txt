cmake_minimum_required(VERSION 3.16)

project(clock_display
    VERSION 1.0.0
    DESCRIPTION "桌面时钟（C / Win32 / GDI+）"
    LANGUAGES C)

if(NOT WIN32)
    message(FATAL_ERROR "clock_display 只支持 Windows（依赖 Win32 / GDI+）。")
endif()

# .rc 要交给 windres / rc.exe 编译，得把 RC 这门语言显式打开
enable_language(RC)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)
set(CMAKE_C_EXTENSIONS OFF)

if(NOT CMAKE_BUILD_TYPE AND NOT CMAKE_CONFIGURATION_TYPES)
    set(CMAKE_BUILD_TYPE Release CACHE STRING "构建类型" FORCE)
    set_property(CACHE CMAKE_BUILD_TYPE PROPERTY STRINGS Debug Release RelWithDebInfo MinSizeRel)
endif()

# ------------------------------------------------------------------ 图标与资源
# 图标是可选的：res/clock.ico 在就带上，不在就照常出 exe。
#
# 路径一律用正斜杠：windres 会把反斜杠当转义序列（\r 变回车、\a 变响铃），
# 而 rc.exe 又不会反转义 \\。正斜杠两边都认，最省事。
set(APP_ICON "${CMAKE_CURRENT_SOURCE_DIR}/res/clock.ico")
set(APP_MANIFEST_RC "${CMAKE_CURRENT_SOURCE_DIR}/res/app.manifest")

if(EXISTS "${APP_ICON}")
    set(APP_ICON_LINE "IDI_APPICON ICON \"${APP_ICON}\"")
else()
    set(APP_ICON_LINE "// 没有 res/clock.ico，本次构建不带图标")
    message(STATUS "没找到 ${APP_ICON}，生成的 exe 用系统默认图标。"
                   "想带上图标，就把一个 .ico 放到 res/clock.ico 再构建。")
endif()

configure_file("${CMAKE_CURRENT_SOURCE_DIR}/res/app.rc.in"
               "${CMAKE_CURRENT_BINARY_DIR}/app.rc"
               @ONLY)

# ------------------------------------------------------------------ 目标
set(APP_SOURCES
    src/main.c
    src/clock.c
    src/menu.c
    src/render.c
    src/surface.c
    src/settings.c
    src/config.c
    src/util.c
    "${CMAKE_CURRENT_BINARY_DIR}/app.rc")

# WIN32 = 窗口子系统：双击启动不会先弹一个命令提示符窗口，正是要的效果
add_executable(clock_display WIN32 ${APP_SOURCES})

target_include_directories(clock_display PRIVATE src)

target_compile_definitions(clock_display PRIVATE
    UNICODE _UNICODE
    WIN32_LEAN_AND_MEAN
    _WIN32_WINNT=0x0601
    WINVER=0x0601
    _CRT_SECURE_NO_WARNINGS)

target_link_libraries(clock_display PRIVATE
    gdiplus     # 抗锯齿文字
    comdlg32    # ChooseColorW：自定义颜色对话框
    user32
    gdi32
    shell32)    # CommandLineToArgvW

# 多配置生成器（Visual Studio）没有 CMAKE_BUILD_TYPE，四个后缀都得设一遍，
# exe 才会统一落在 <build>/bin 下，找起来方便。
foreach(_cfg "" _DEBUG _RELEASE _RELWITHDEBINFO _MINSIZEREL)
    set_target_properties(clock_display PROPERTIES
        RUNTIME_OUTPUT_DIRECTORY${_cfg} "${CMAKE_BINARY_DIR}/bin")
endforeach()

if(MSVC)
    # /utf-8：源码是 UTF-8，里面一串中文字面量，不加这个 MSVC 会按 GBK 解析
    target_compile_options(clock_display PRIVATE /utf-8 /W3)
    # 清单已经由 app.rc 以 RT_MANIFEST 提供，关掉链接器自动生成的那一份，免得冲突
    target_link_options(clock_display PRIVATE /MANIFEST:NO)
else()
    target_compile_options(clock_display PRIVATE -Wall -Wextra)

    # MinGW 默认把 libgcc_s_seh-1.dll 做成动态依赖，exe 拷到没装 MSYS2 的电脑上
    # 就起不来。静态链进去之后，剩下的依赖只有 kernel32/user32/gdiplus 这些
    # 系统自带的 DLL —— 真正做到「拷过去双击就能用」。
    target_link_options(clock_display PRIVATE -static -static-libgcc)
endif()
