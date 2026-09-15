//! 屏幕条纹生成器 —— Rust / Win32 实现（零依赖）
//!
//! 模块划分：
//! - `win`      Win32 FFI 层（手写 extern 声明）
//! - `model`    纯逻辑：随机数 / 条纹生成 / 区域模型 / 参数
//! - `layer`    条纹覆盖层（色键透明 + 鼠标穿透 + 逐线闪烁）
//! - `selector` 全屏圈选遮罩
//! - `app`      主控制窗口（自绘 GUI + 状态机 + 老板键）

// 无控制台窗口；诊断信息写入 screen_stripe.log
#![windows_subsystem = "windows"]

mod app;
mod disclaimer;
mod layer;
mod model;
mod selector;
mod win;

use std::io::Write;
use model::{ColorMode, Orientation};

/// 日志开关（仅在传入 --log 时打开）
pub(crate) static LOG_ON: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// 诊断日志路径：固定写到 exe 同目录，避免受启动时工作目录影响
fn log_path() -> std::path::PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("screen_stripe.log")))
        .unwrap_or_else(|| std::path::PathBuf::from("screen_stripe.log"))
}

/// 无条件写日志（panic 追踪用）
pub(crate) fn log_force(msg: &str) {
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path())
    {
        let _ = writeln!(f, "{msg}");
    }
}

/// 追加一行诊断日志（仅 --log 时生效）
pub(crate) fn log(msg: &str) {
    if LOG_ON.load(std::sync::atomic::Ordering::Relaxed) {
        log_force(msg);
    }
}

fn main() {
    unsafe { win::SetProcessDpiAwarenessContext(win::DPI_PER_MONITOR_AWARE_V2) };

    // GUI 程序没有控制台，崩溃必须留痕
    std::panic::set_hook(Box::new(|info| {
        log_force(&format!("PANIC: {info}"));
    }));

    let args: Vec<String> = std::env::args().collect();
    let arg_val = |key: &str| -> Option<String> {
        args.iter()
            .position(|a| a == key)
            .and_then(|i| args.get(i + 1).cloned())
    };
    let log_on = args.iter().any(|a| a == "--log");
    LOG_ON.store(log_on, std::sync::atomic::Ordering::Relaxed);
    if log_on {
        let _ = std::fs::remove_file(log_path());
    }

    // --selftest <秒>：程序化注入一个区域、生成条纹、写诊断日志、若干秒后自动退出
    // （防截图由主窗口的「防截图」勾选项控制，默认关闭 = 可被截图捕获）
    let selftest: Option<u64> = arg_val("--selftest").and_then(|v| v.parse().ok());
    // --orient h / --mode black|white / --flick
    let want_orient = arg_val("--orient").map(|v| {
        if v.starts_with('h') {
            Orientation::Horizontal
        } else {
            Orientation::Vertical
        }
    });
    let want_mode = arg_val("--mode").map(|v| match v.as_str() {
        "black" => ColorMode::Black,
        "white" => ColorMode::White,
        _ => ColorMode::Multi,
    });
    let want_flick = args.iter().any(|a| a == "--flick");

    unsafe {
        let inst = win::GetModuleHandleW(std::ptr::null());

        // ---- 启动免责声明：置顶模态，不同意则立即退出，不创建任何其它窗口 ----
        // 没有任何命令行开关可以跳过：必须由用户点击「同意」或按回车。
        // （自动化测试通过模拟点击真实按钮来通过，而不是靠后门参数）
        disclaimer::create(inst);
        let agreed = disclaimer::ask();
        disclaimer::destroy();
        if !agreed {
            log("用户不同意免责声明，程序立即退出");
            return;
        }
        log("用户已同意免责声明");

        let _layer = layer::create(inst);
        let _selector = selector::create(inst);
        let app_hwnd = app::create(inst);
        if app_hwnd == 0 {
            if log_on {
                log("创建主窗口失败");
            }
            return;
        }
        app::show(app_hwnd);

        if args.iter().any(|a| a == "--boss") {
            // 预置老板键 Ctrl+Alt+B（等价于用户在界面上设置）
            app::debug_set_hotkey(win::MOD_CONTROL | win::MOD_ALT, 'B' as u32);
        }

        if let Some(secs) = selftest {
            let (vx, _vy, _vw, _vh) = win::virtual_screen();
            app::debug_add_region(620 + vx, 300, 600, 420);
            app::debug_set(want_orient, Some(want_flick), want_mode);
            log(&format!(
                "selftest: {} | 线条总数={} 可见={} | 防截图={} | 参数 orient={:?} mode={:?} flick={}",
                app::debug_state(),
                layer::total_line_count(),
                layer::visible_line_count(),
                layer::capture_excluded(),
                want_orient,
                want_mode,
                want_flick
            ));
            // 若干秒后自行关闭
            let hwnd = app_hwnd;
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(secs));
                win::PostMessageW(hwnd, win::WM_CLOSE, 0, 0);
            });
        }

        let mut msg: win::MSG = std::mem::zeroed();
        while win::GetMessageW(&mut msg, 0, 0, 0) > 0 {
            win::TranslateMessage(&msg);
            win::DispatchMessageW(&msg);
        }

        app::destroy();
        layer::destroy();
        selector::destroy();
        if log_on {
            log("已退出");
        }
    }
}
