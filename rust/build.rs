// 把资源编进 exe：清单（comctl32 v6 现代菜单 + DPI 感知）、图标、版本信息。
// 对应 C 版 CMakeLists.txt 里 windres 编 res/app.rc 的那一步。
fn main() {
    println!("cargo:rerun-if-changed=res/app.rc");
    println!("cargo:rerun-if-changed=../res/app.manifest");
    println!("cargo:rerun-if-changed=../res/clock.ico");

    // manifest_required：找不到 rc.exe / llvm-rc 时当场报错，
    // 而不是悄悄出一个没有清单的 exe（那样右键菜单是老样式、高 DPI 下发糊）。
    embed_resource::compile("res/app.rc", embed_resource::NONE)
        .manifest_required()
        .unwrap();
}
