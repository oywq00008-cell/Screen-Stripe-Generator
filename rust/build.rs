//! 构建脚本：把 `assets/app.rc` 编译成 `.res` 并链接进 exe，使程序带上图标。
//!
//! 保持零依赖 —— 直接调用 Windows SDK 自带的 `rc.exe`，不引入任何 crate。
//! 若找不到 rc.exe 则跳过图标（只提示警告，不影响功能与构建）。

use std::path::{Path, PathBuf};
use std::process::Command;

fn find_rc() -> Option<PathBuf> {
    // 1) PATH 中直接有
    if let Ok(out) = Command::new("where").arg("rc.exe").output() {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout);
            if let Some(line) = s.lines().next() {
                let p = PathBuf::from(line.trim());
                if p.is_file() {
                    return Some(p);
                }
            }
        }
    }
    // 2) 常见 SDK 安装位置（本机 SDK 装在 H 盘，故遍历多个盘符）
    let mut roots: Vec<PathBuf> = Vec::new();
    for drive in ["C", "D", "E", "F", "G", "H"] {
        roots.push(PathBuf::from(format!(
            "{drive}:/Program Files (x86)/Windows Kits/10/bin"
        )));
        roots.push(PathBuf::from(format!("{drive}:/Windows Kits/10/bin")));
    }
    for root in roots {
        if let Ok(entries) = std::fs::read_dir(&root) {
            let mut versions: Vec<PathBuf> =
                entries.filter_map(|e| e.ok()).map(|e| e.path()).collect();
            versions.sort();
            versions.reverse();
            for v in versions {
                for arch in ["x64", "x86"] {
                    let p = v.join(arch).join("rc.exe");
                    if p.is_file() {
                        return Some(p);
                    }
                }
            }
        }
    }
    None
}

fn main() {
    println!("cargo:rerun-if-changed=assets/app.rc");
    println!("cargo:rerun-if-changed=assets/app.ico");

    let rc = match find_rc() {
        Some(p) => p,
        None => {
            println!("cargo:warning=未找到 rc.exe（Windows SDK），跳过图标资源；功能不受影响");
            return;
        }
    };

    let out_dir = std::env::var("OUT_DIR").unwrap_or_else(|_| ".".into());
    let res = Path::new(&out_dir).join("app.res");
    let status = Command::new(&rc)
        .current_dir("assets")
        .args(["/nologo", "/fo", &res.to_string_lossy(), "app.rc"])
        .status();

    match status {
        Ok(s) if s.success() => {
            println!("cargo:rustc-link-arg-bins={}", res.display());
            println!("cargo:warning=已嵌入图标资源（rc.exe: {}）", rc.display());
        }
        _ => println!("cargo:warning=rc.exe 执行失败，跳过图标资源"),
    }
}
