//! 主控制窗口：单窗口自绘 GUI（标题 / 按钮 / 勾选项 / 方向开关 / 老板键 / 状态 / 预览 / 说明）。
//!
//! 之所以不逐个创建 Win32 子控件，而是整窗自绘 + 命中测试：
//! 外观完全可控（能 1:1 还原既定配色与圆角），代码量更少，也不受原生控件外观限制。

#![allow(dead_code)]

use std::cell::RefCell;

use crate::model::*;
use crate::win::*;
use crate::{layer, selector};

pub const APP_CLASS: &str = "ScreenStripeAppW";

// ---------------------------------------------------------------- 布局常量

const CW: i32 = 430; // 客户区宽
const CH: i32 = 852; // 客户区高（含 8 行帮助文案）
const X0: i32 = 18; // 内容左边距
const W: i32 = 394; // 内容宽

// 配色
const C_BG: COLORREF = rgb(244, 245, 247);
const C_TITLE: COLORREF = rgb(31, 35, 40);
const C_MUTED: COLORREF = rgb(107, 114, 128);
const C_BLUE: COLORREF = rgb(37, 99, 235);
const C_BLUE_H: COLORREF = rgb(29, 78, 216);
const C_GREEN: COLORREF = rgb(22, 163, 74);
const C_GREEN_H: COLORREF = rgb(21, 128, 61);
const C_RED: COLORREF = rgb(220, 38, 38);
const C_RED_H: COLORREF = rgb(185, 28, 28);
const C_WHITE: COLORREF = rgb(255, 255, 255);
const C_BORDER: COLORREF = rgb(107, 114, 128);
const C_BOX_H: COLORREF = rgb(238, 242, 247);
const C_BOX_ON: COLORREF = rgb(230, 241, 251);
const C_WARN: COLORREF = rgb(185, 28, 28);
const C_CANVAS_BG: COLORREF = rgb(249, 250, 251);
const C_BAND: COLORREF = rgb(254, 202, 202);
const C_BAND_EDGE: COLORREF = rgb(220, 38, 38);

// 命中目标
const HIT_NONE: i32 = 0;
const HIT_SEL: i32 = 1;
const HIT_GEN0: i32 = 2;
const HIT_GEN1: i32 = 3;
const HIT_GEN2: i32 = 4;
const HIT_CLOSE: i32 = 5;
const HIT_FLICK: i32 = 6;
const HIT_ORIENT: i32 = 7;
const HIT_BOSS: i32 = 8;

const HOTKEY_ID: i32 = 1;

fn r_sel() -> RECT {
    RECT::new(X0, 70, X0 + W, 112)
}
fn r_gen(i: i32) -> RECT {
    let w = 127;
    let x = X0 + i * (w + 6);
    RECT::new(x, 138, x + w, 176)
}
fn r_close() -> RECT {
    RECT::new(X0, 184, X0 + W, 226)
}
fn r_flick() -> RECT {
    RECT::new(X0, 234, X0 + W, 256)
}
fn r_orient() -> RECT {
    RECT::new(X0, 262, X0 + W, 298)
}
fn r_boss() -> RECT {
    RECT::new(X0, 306, X0 + W, 342)
}
fn r_preview() -> RECT {
    RECT::new(X0, 412, X0 + W, 562)
}

// ---------------------------------------------------------------- 应用状态

struct App {
    hwnd: HWND,
    regions: Vec<Region>,
    orientation: Orientation,
    mode: ColorMode,
    flicker: bool,
    stripe_visible: bool,
    hover: i32,
    capturing: bool,
    boss_mods: UINT,
    boss_vk: u32,
    boss_set: bool,
    hint: Option<String>,
    // GDI 资源
    bg: HBRUSH,
    white: HBRUSH,
    border: HBRUSH,
    band: HBRUSH,
    band_edge: HBRUSH,
    canvas_bg: HBRUSH,
    f_title: HFONT,
    f_btn: HFONT,
    f_body: HFONT,
    f_small: HFONT,
    f_help: HFONT,
    mem_dc: HDC,
    mem_bmp: HBITMAP,
}

thread_local! {
    static APP: RefCell<Option<App>> = RefCell::new(None);
}

impl App {
    // ---- 文字与图形绘制辅助 -------------------------------------------------
    fn text(&self, hdc: HDC, s: &str, r: RECT, font: HFONT, color: COLORREF, flags: UINT) {
        unsafe {
            SetBkMode(hdc, TRANSPARENT);
            SetTextColor(hdc, color);
            let old = SelectObject(hdc, font);
            let mut rr = r;
            let w = wide(s);
            DrawTextW(hdc, w.as_ptr(), -1, &mut rr, flags | DT_NOPREFIX);
            SelectObject(hdc, old);
        }
    }

    fn round_fill(&self, hdc: HDC, r: RECT, color: COLORREF, radius: i32) {
        unsafe {
            let b = CreateSolidBrush(color);
            let ob = SelectObject(hdc, b);
            let op = SelectObject(hdc, GetStockObject(NULL_PEN));
            RoundRect(hdc, r.left, r.top, r.right, r.bottom, radius, radius);
            SelectObject(hdc, op);
            SelectObject(hdc, ob);
            DeleteObject(b);
        }
    }

    fn fill(&self, hdc: HDC, r: RECT, brush: HBRUSH) {
        unsafe { FillRect(hdc, &r, brush) };
    }

    fn frame(&self, hdc: HDC, r: RECT, brush: HBRUSH) {
        unsafe { FrameRect(hdc, &r, brush) };
    }

    fn button(&self, hdc: HDC, r: RECT, label: &str, base: COLORREF, hot: COLORREF, on: bool) {
        let c = if on { hot } else { base };
        self.round_fill(hdc, r, c, 14);
        self.text(hdc, label, r, self.f_btn, C_WHITE, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
    }

    /// 白底 + 1px 边框 + 居中文字的「开关条」
    fn box_row(&self, hdc: HDC, r: RECT, label: &str, selected: bool, on: bool) {
        let bg = if selected {
            C_BOX_ON
        } else if on {
            C_BOX_H
        } else {
            C_WHITE
        };
        unsafe {
            let b = CreateSolidBrush(bg);
            self.fill(hdc, r, b);
            DeleteObject(b);
        }
        self.frame(hdc, r, self.border);
        self.text(hdc, label, r, self.f_small, C_TITLE,
                  DT_CENTER | DT_VCENTER | DT_SINGLELINE);
    }

    // ---- 命中测试 -----------------------------------------------------------
    fn hit(&self, x: i32, y: i32) -> i32 {
        if r_sel().contains(x, y) {
            return HIT_SEL;
        }
        for i in 0..3 {
            if r_gen(i).contains(x, y) {
                return HIT_GEN0 + i;
            }
        }
        if r_close().contains(x, y) {
            return HIT_CLOSE;
        }
        if r_flick().contains(x, y) {
            return HIT_FLICK;
        }
        if r_orient().contains(x, y) {
            return HIT_ORIENT;
        }
        if r_boss().contains(x, y) {
            return HIT_BOSS;
        }
        HIT_NONE
    }

    // ---- 绘制 ---------------------------------------------------------------
    fn paint(&self, hdc: HDC) {
        let client = RECT::new(0, 0, CW, CH);
        self.fill(hdc, client, self.bg);

        self.text(hdc, "屏幕条纹生成器", RECT::new(X0, 14, X0 + W, 40),
                  self.f_title, C_TITLE, DT_LEFT | DT_BOTTOM | DT_SINGLELINE);
        self.text(hdc, "纯恶作剧用途，不会对显示器或系统产生任何实际影响",
                  RECT::new(X0, 42, X0 + W, 60), self.f_small, C_MUTED,
                  DT_LEFT | DT_VCENTER | DT_SINGLELINE);

        // 开始划定区域
        self.button(hdc, r_sel(), "开始划定区域", C_BLUE, C_BLUE_H, self.hover == HIT_SEL);

        // 生成条纹（三色）
        self.text(hdc, "生成条纹 · 选一种颜色，点击即生成",
                  RECT::new(X0, 118, X0 + W, 134), self.f_small, C_MUTED,
                  DT_LEFT | DT_VCENTER | DT_SINGLELINE);
        for (i, (mode, label)) in [(ColorMode::Multi, "多色"), (ColorMode::Black, "纯黑"),
                                   (ColorMode::White, "纯白")].iter().enumerate() {
            let _ = mode;
            self.button(hdc, r_gen(i as i32), label, C_GREEN, C_GREEN_H,
                        self.hover == HIT_GEN0 + i as i32);
        }

        // 关闭条纹
        let close_label = if self.hint.is_some() { "关闭条纹（保留标记）" } else { "关闭条纹（保留标记）" };
        self.button(hdc, r_close(), close_label, C_RED, C_RED_H, self.hover == HIT_CLOSE);

        // 随机闪烁勾选项
        let fr = r_flick();
        let bx = RECT::new(fr.left, fr.top + 3, fr.left + 15, fr.top + 18);
        self.fill(hdc, bx, self.white);
        let on = self.flicker;
        if on {
            self.round_fill(hdc, bx, C_BLUE, 6);
            // 打勾
            unsafe {
                let pen = CreatePen(0, 2, C_WHITE);
                let op = SelectObject(hdc, pen);
                MoveToEx(hdc, bx.left + 4, bx.top + 8, std::ptr::null_mut());
                LineTo(hdc, bx.left + 6, bx.top + 11);
                LineTo(hdc, bx.left + 11, bx.top + 4);
                SelectObject(hdc, op);
                DeleteObject(pen);
            }
        } else {
            self.round_fill(hdc, bx, C_WHITE, 6);
            self.frame(hdc, bx, self.border);
        }
        self.text(hdc, "随机闪烁（几条线条，间隔 1~5 秒，单次 0.1~3 秒）",
                  RECT::new(bx.right + 8, fr.top, X0 + W, fr.bottom), self.f_small, C_TITLE,
                  DT_LEFT | DT_VCENTER | DT_SINGLELINE);

        // 条纹方向开关
        let orient_label = format!("条纹方向：{}（点击切换）", self.orientation.name());
        self.box_row(hdc, r_orient(), &orient_label,
                     self.orientation == Orientation::Horizontal, self.hover == HIT_ORIENT);

        // 老板键
        let boss_label = if self.capturing {
            "请按下要绑定的组合键…（Esc 取消）".to_string()
        } else if self.boss_set {
            format!("老板键：{}（点击重设）", self.boss_text())
        } else {
            "老板键：未设置（点击设置）".to_string()
        };
        self.box_row(hdc, r_boss(), &boss_label, self.capturing, self.hover == HIT_BOSS);

        // 状态
        self.text(hdc, &format!("已标记区域：{} 个（已记忆）", self.regions.len()),
                  RECT::new(X0, 350, X0 + W, 368), self.f_body, C_TITLE,
                  DT_LEFT | DT_VCENTER | DT_SINGLELINE);
        match &self.hint {
            Some(h) => self.text(hdc, h, RECT::new(X0, 368, X0 + W, 386), self.f_body, C_WARN,
                                 DT_LEFT | DT_VCENTER | DT_SINGLELINE),
            None => {
                let s = if self.stripe_visible {
                    format!("条纹：显示中（{} · {}）　　闪烁：{}", self.mode.name(),
                            self.orientation.name(), if self.flicker { "开" } else { "关" })
                } else {
                    format!("条纹：已关闭　　闪烁：{}", if self.flicker { "开" } else { "关" })
                };
                self.text(hdc, &s, RECT::new(X0, 368, X0 + W, 386), self.f_body, C_TITLE,
                          DT_LEFT | DT_VCENTER | DT_SINGLELINE);
            }
        }

        // 预览
        self.text(hdc, "虚拟桌面预览（红框 = 条纹覆盖范围）",
                  RECT::new(X0, 394, X0 + W, 410), self.f_small, C_MUTED,
                  DT_LEFT | DT_VCENTER | DT_SINGLELINE);
        self.draw_preview(hdc, r_preview());

        // 区域明细
        self.text(hdc, "区域明细", RECT::new(X0, 570, X0 + W, 586), self.f_small, C_MUTED,
                  DT_LEFT | DT_VCENTER | DT_SINGLELINE);
        let detail = if self.regions.is_empty() {
            "（暂无标记区域，请点「开始划定区域」）".to_string()
        } else {
            self.regions
                .iter()
                .take(3)
                .map(|r| r.summary(self.orientation))
                .collect::<Vec<_>>()
                .join("\n")
        };
        self.text(hdc, &detail, RECT::new(X0, 588, X0 + W, 652), self.f_small, rgb(55, 65, 81),
                  DT_LEFT | DT_TOP | DT_WORDBREAK);

        // 说明
        let help = [
            "· 先点「开始划定区域」拖动画圈（可圈多个）",
            "   右键 / 回车 / Esc 结束圈选并返回",
            "· 点「多色 / 纯黑 / 纯白」即按该颜色生成条纹",
            "· 「关闭条纹」只关显示，标记会记住；重新圈选替换旧标记",
            "· 「随机闪烁」：几条线隔 1~5 秒闪一次，单次消失 0.1~3 秒",
            "· 「条纹方向」：竖条纹贯穿屏幕高度，横条纹贯穿屏幕宽度",
            "· 「老板键」：设置后按组合键隐藏 / 恢复本窗口，条纹不受影响",
            "· 任意界面按 Esc 可安全退出并清理所有覆盖层",
        ].join("
");
        self.text(hdc, &help, RECT::new(X0, 660, X0 + W, 840), self.f_help, C_MUTED,
                  DT_LEFT | DT_TOP | DT_WORDBREAK);
    }

    fn draw_preview(&self, hdc: HDC, r: RECT) {
        self.fill(hdc, r, self.white);
        self.frame(hdc, r, self.border);
        let inner = RECT::new(r.left + 6, r.top + 6, r.right - 6, r.bottom - 6);
        self.fill(hdc, inner, self.canvas_bg);

        let (vx, vy, vw, vh) = virtual_screen();
        if vw <= 0 || vh <= 0 {
            return;
        }
        let sx = (inner.w() as f32) / (vw as f32);
        let sy = (inner.h() as f32) / (vh as f32);
        for reg in &self.regions {
            let c = match self.orientation {
                Orientation::Vertical => {
                    let x0 = inner.left + ((reg.x - vx) as f32 * sx) as i32;
                    let x1 = inner.left + ((reg.x + reg.w - vx) as f32 * sx) as i32;
                    RECT::new(x0, inner.top + 1, x1.max(x0 + 2), inner.bottom - 1)
                }
                Orientation::Horizontal => {
                    let y0 = inner.top + ((reg.y - vy) as f32 * sy) as i32;
                    let y1 = inner.top + ((reg.y + reg.h - vy) as f32 * sy) as i32;
                    RECT::new(inner.left + 1, y0, inner.right - 1, y1.max(y0 + 2))
                }
            };
            self.fill(hdc, c, self.band);
            self.frame(hdc, c, self.band_edge);
        }
    }

    // ---- 老板键文本 ---------------------------------------------------------
    fn boss_text(&self) -> String {
        let mut s = String::new();
        if self.boss_mods & MOD_CONTROL != 0 {
            s.push_str("Ctrl+");
        }
        if self.boss_mods & MOD_ALT != 0 {
            s.push_str("Alt+");
        }
        if self.boss_mods & MOD_SHIFT != 0 {
            s.push_str("Shift+");
        }
        if self.boss_mods & MOD_WIN != 0 {
            s.push_str("Win+");
        }
        s.push_str(&vk_name(self.boss_vk));
        s
    }

    // ---- 动作 ---------------------------------------------------------------
    /// 只标脏，不在此处同步 UpdateWindow —— 否则会在持有 RefCell 借用时
    /// 同步派发 WM_PAINT，触发重复借用而崩。
    fn invalidate(&self) {
        unsafe { InvalidateRect(self.hwnd, std::ptr::null(), 0) };
    }

    /// 让主窗口保持在覆盖层之上
    fn raise_above_layer(&self) {
        unsafe {
            SetWindowPos(self.hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
        }
    }

    fn generate(&mut self, mode: ColorMode) {
        self.mode = mode;
        if self.regions.is_empty() {
            self.hint = Some("请先点「开始划定区域」圈出条纹的覆盖范围".into());
            self.invalidate();
            return;
        }
        self.hint = None;
        layer::show(&self.regions, self.orientation, self.mode, self.flicker);
        self.stripe_visible = true;
        self.raise_above_layer();
        self.invalidate();
    }

    fn close_stripes(&mut self) {
        layer::hide();
        self.stripe_visible = false;
        self.hint = None;
        self.invalidate();
    }

    fn start_select(&mut self) {
        let restore = self.stripe_visible;
        if self.stripe_visible {
            layer::hide();
            self.stripe_visible = false;
        }
        self.hint = None;
        self.invalidate();

        selector::begin(&self.regions, self.orientation);
        selector::run_modal();
        let new_regions = selector::take_result();
        if !new_regions.is_empty() {
            self.regions = new_regions; // 重新画 → 替换旧标记
        }
        unsafe {
            SetForegroundWindow(self.hwnd);
            SetFocus(self.hwnd);
        }
        if restore && !self.regions.is_empty() {
            self.generate(self.mode);
        } else {
            layer::hide();
            self.stripe_visible = false;
            self.invalidate();
        }
    }

    fn toggle_orientation(&mut self) {
        self.orientation = match self.orientation {
            Orientation::Vertical => Orientation::Horizontal,
            Orientation::Horizontal => Orientation::Vertical,
        };
        if self.stripe_visible && !self.regions.is_empty() {
            layer::rebuild(&self.regions, self.orientation, self.mode); // 立即重绘
            self.raise_above_layer();
        }
        self.invalidate();
    }

    fn toggle_flicker(&mut self) {
        self.flicker = !self.flicker;
        if self.stripe_visible {
            layer::set_flicker(self.flicker);
        }
        self.invalidate();
    }

    /// 老板键：隐藏 / 恢复本窗口（条纹不受影响）
    fn boss_toggle(&mut self) {
        unsafe {
            if IsWindowVisible(self.hwnd) != 0 {
                ShowWindow(self.hwnd, SW_HIDE);
            } else {
                ShowWindow(self.hwnd, SW_SHOW);
                self.raise_above_layer();
                SetForegroundWindow(self.hwnd);
                self.invalidate();
            }
        }
    }

    /// 捕获组合键并注册全局热键
    fn capture_hotkey(&mut self, vk: u32) {
        if vk == VK_ESCAPE as u32 {
            self.capturing = false;
            self.invalidate();
            return;
        }
        // 忽略纯修饰键本身
        if vk == VK_CONTROL as u32 || vk == VK_SHIFT as u32 || vk == VK_MENU as u32
            || vk == VK_LWIN as u32
        {
            return;
        }
        let mut mods: UINT = 0;
        unsafe {
            if GetKeyState(VK_CONTROL) < 0 {
                mods |= MOD_CONTROL;
            }
            if GetKeyState(VK_MENU) < 0 {
                mods |= MOD_ALT;
            }
            if GetKeyState(VK_SHIFT) < 0 {
                mods |= MOD_SHIFT;
            }
            if GetKeyState(VK_LWIN) < 0 {
                mods |= MOD_WIN;
            }
        }
        if mods == 0 {
            self.hint = Some("快捷键需至少包含一个修饰键（Ctrl / Alt / Shift / Win）".into());
            self.capturing = false;
            self.invalidate();
            return;
        }
        let old = (self.boss_mods, self.boss_vk, self.boss_set);
        unsafe {
            if self.boss_set {
                UnregisterHotKey(self.hwnd, HOTKEY_ID);
            }
            if RegisterHotKey(self.hwnd, HOTKEY_ID, mods, vk) != 0 {
                self.boss_mods = mods;
                self.boss_vk = vk;
                self.boss_set = true;
                self.hint = None;
            } else {
                // 注册失败（多半被其它程序占用）→ 回滚旧快捷键
                if old.2 {
                    RegisterHotKey(self.hwnd, HOTKEY_ID, old.0, old.1);
                }
                self.hint = Some(format!("「{}」已被其它程序占用，请换一组", self.boss_text_of(mods, vk)));
            }
        }
        self.capturing = false;
        self.invalidate();
    }

    fn boss_text_of(&self, mods: UINT, vk: u32) -> String {
        let mut s = String::new();
        if mods & MOD_CONTROL != 0 {
            s.push_str("Ctrl+");
        }
        if mods & MOD_ALT != 0 {
            s.push_str("Alt+");
        }
        if mods & MOD_SHIFT != 0 {
            s.push_str("Shift+");
        }
        if mods & MOD_WIN != 0 {
            s.push_str("Win+");
        }
        s.push_str(&vk_name(vk));
        s
    }
}

/// 虚拟键 → 可读名称
fn vk_name(vk: u32) -> String {
    match vk {
        0x30..=0x39 | 0x41..=0x5A => (vk as u8 as char).to_string(),
        0x70..=0x7B => format!("F{}", vk - 0x6F),
        0x20 => "Space".into(),
        0x0D => "Enter".into(),
        0x09 => "Tab".into(),
        0x25 => "Left".into(),
        0x26 => "Up".into(),
        0x27 => "Right".into(),
        0x28 => "Down".into(),
        0x24 => "Home".into(),
        0x23 => "End".into(),
        0x21 => "PageUp".into(),
        0x22 => "PageDown".into(),
        0x2D => "Insert".into(),
        0x2E => "Delete".into(),
        _ => format!("0x{vk:02X}"),
    }
}

// ---------------------------------------------------------------- 窗口过程

unsafe extern "system" fn app_proc(hwnd: HWND, msg: UINT, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = unsafe { std::mem::zeroed() };
            let hdc = unsafe { BeginPaint(hwnd, &mut ps) };
            APP.with(|a| {
                // try_borrow：若此刻正被外层借用（同步重入），跳过本次绘制，
                // 失效区域仍然保留，稍后消息循环里会补画。
                let mut guard = match a.try_borrow_mut() {
                    Ok(g) => g,
                    Err(_) => {
                        unsafe { EndPaint(hwnd, &ps) };
                        return;
                    }
                };
                if let Some(app) = guard.as_mut() {
                    if app.hwnd != hwnd {
                        unsafe { EndPaint(hwnd, &ps) };
                        return;
                    }
                    // 双缓冲：先画到内存 DC，再一次性贴到窗口，避免闪烁
                    if app.mem_dc == 0 {
                        app.mem_dc = unsafe { CreateCompatibleDC(hdc) };
                        app.mem_bmp = unsafe { CreateCompatibleBitmap(hdc, CW, CH) };
                        unsafe { SelectObject(app.mem_dc, app.mem_bmp) };
                    }
                    app.paint(app.mem_dc);
                    unsafe { BitBlt(hdc, 0, 0, CW, CH, app.mem_dc, 0, 0, SRCCOPY) };
                }
            });
            unsafe { EndPaint(hwnd, &ps) };
            return 0;
        }
        WM_ERASEBKGND => return 1,
        WM_MOUSEMOVE => {
            let (x, y) = cursor_from_lparam(lp);
            let mut need = false;
            APP.with(|a| {
                let mut guard = match a.try_borrow_mut() {
                    Ok(g) => g,
                    Err(_) => return,
                };
                if let Some(app) = guard.as_mut() {
                    let h = app.hit(x, y);
                    if h != app.hover {
                        app.hover = h;
                        need = true;
                    }
                    let mut tme = TRACKMOUSEEVENT {
                        cb_size: std::mem::size_of::<TRACKMOUSEEVENT>() as DWORD,
                        dw_flags: TME_LEAVE,
                        hwnd_track: hwnd,
                        dw_hover_time: 0,
                    };
                    unsafe { TrackMouseEvent(&mut tme) };
                }
            });
            if need {
                unsafe { InvalidateRect(hwnd, std::ptr::null(), 0) };
            }
            return 0;
        }
        WM_MOUSELEAVE => {
            APP.with(|a| {
                let mut guard = match a.try_borrow_mut() {
                    Ok(g) => g,
                    Err(_) => return,
                };
                if let Some(app) = guard.as_mut() {
                    app.hover = HIT_NONE;
                }
            });
            unsafe { InvalidateRect(hwnd, std::ptr::null(), 0) };
            return 0;
        }
        WM_SETCURSOR => {
            let mut hand = false;
            APP.with(|a| {
                let mut guard = match a.try_borrow_mut() {
                    Ok(g) => g,
                    Err(_) => return,
                };
                if let Some(app) = guard.as_mut() {
                    hand = app.hover != HIT_NONE;
                }
            });
            if (lp & 0xFFFF) as i32 == HTCLIENT as i32 {
                let cur = unsafe { LoadCursorW(0, if hand { IDC_HAND } else { IDC_ARROW }) };
                unsafe { SetCursor(cur) };
                return 1;
            }
        }
        WM_LBUTTONDOWN => {
            let (x, y) = cursor_from_lparam(lp);
            crate::log(&format!("点击 client=({x},{y})"));
            APP.with(|a| {
                let mut guard = match a.try_borrow_mut() {
                    Ok(g) => g,
                    Err(_) => return,
                };
                if let Some(app) = guard.as_mut() {
                    let h = app.hit(x, y);
                    crate::log(&format!("  命中目标={h}"));
                    match h {
                        HIT_SEL => app.start_select(),
                        HIT_GEN0 => app.generate(ColorMode::Multi),
                        HIT_GEN1 => app.generate(ColorMode::Black),
                        HIT_GEN2 => app.generate(ColorMode::White),
                        HIT_CLOSE => app.close_stripes(),
                        HIT_FLICK => app.toggle_flicker(),
                        HIT_ORIENT => app.toggle_orientation(),
                        HIT_BOSS => {
                            app.capturing = !app.capturing;
                            app.hint = None;
                            app.invalidate();
                        }
                        _ => {}
                    }
                }
            });
            return 0;
        }
        WM_KEYDOWN => {
            let vk = wp as u32;
            let mut quit = false;
            APP.with(|a| {
                let mut guard = match a.try_borrow_mut() {
                    Ok(g) => g,
                    Err(_) => return,
                };
                if let Some(app) = guard.as_mut() {
                    if app.capturing {
                        app.capture_hotkey(vk);
                    } else if vk == VK_ESCAPE as u32 {
                        quit = true;
                    }
                }
            });
            if quit {
                unsafe { DestroyWindow(hwnd) };
            }
            return 0;
        }
        WM_HOTKEY => {
            if wp as i32 == HOTKEY_ID {
                APP.with(|a| {
                    let mut guard = match a.try_borrow_mut() {
                    Ok(g) => g,
                    Err(_) => return,
                };
                if let Some(app) = guard.as_mut() {
                        app.boss_toggle();
                    }
                });
            }
            return 0;
        }
        WM_CLOSE => {
            unsafe { DestroyWindow(hwnd) };
            return 0;
        }
        WM_DESTROY => {
            APP.with(|a| {
                let mut guard = match a.try_borrow_mut() {
                    Ok(g) => g,
                    Err(_) => return,
                };
                if let Some(app) = guard.as_mut() {
                    unsafe {
                        if app.boss_set {
                            UnregisterHotKey(app.hwnd, HOTKEY_ID);
                        }
                        if app.mem_dc != 0 {
                            DeleteDC(app.mem_dc);
                            DeleteObject(app.mem_bmp);
                            app.mem_dc = 0;
                        }
                    }
                }
            });
            unsafe { PostQuitMessage(0) };
            return 0;
        }
        _ => {}
    }
    unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
}

// ---------------------------------------------------------------- 创建与销毁

pub fn create(inst: HINSTANCE) -> HWND {
    let class = wide(APP_CLASS);
    let wc = WNDCLASSW {
        style: 0,
        lpfn_wnd_proc: Some(app_proc),
        cb_cls_extra: 0,
        cb_wnd_extra: 0,
        h_instance: inst,
        h_icon: unsafe { LoadIconW(inst, 1 as *const u16) },
        h_cursor: unsafe { LoadCursorW(0, IDC_ARROW) },
        hbr_background: 0,
        lpsz_menu_name: std::ptr::null(),
        lpsz_class_name: class.as_ptr(),
    };
    unsafe { RegisterClassW(&wc) };

    // 按客户区尺寸反算窗口尺寸
    let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX;
    let mut rect = RECT::new(0, 0, CW, CH);
    unsafe { AdjustWindowRectEx(&mut rect, style, 0, 0) };

    let title = wide("屏幕条纹生成器");
    let hwnd = unsafe {
        CreateWindowExW(
            0,
            class.as_ptr(),
            title.as_ptr(),
            style,
            60,
            40,
            rect.w(),
            rect.h(),
            0,
            0,
            inst,
            std::ptr::null_mut(),
        )
    };
    if hwnd == 0 {
        return 0;
    }

    let face = wide("Microsoft YaHei UI");
    let (bg, white, border, band, band_edge, canvas_bg) = unsafe {
        (
            CreateSolidBrush(C_BG),
            CreateSolidBrush(C_WHITE),
            CreateSolidBrush(C_BORDER),
            CreateSolidBrush(C_BAND),
            CreateSolidBrush(C_BAND_EDGE),
            CreateSolidBrush(C_CANVAS_BG),
        )
    };
    let (f_title, f_btn, f_body, f_small, f_help) = unsafe {
        (
            CreateFontW(-22, 0, 0, 0, FW_BOLD, 0, 0, 0, DEFAULT_CHARSET, 0, 0,
                        CLEARTYPE_QUALITY, 0, face.as_ptr()),
            CreateFontW(-16, 0, 0, 0, FW_BOLD, 0, 0, 0, DEFAULT_CHARSET, 0, 0,
                        CLEARTYPE_QUALITY, 0, face.as_ptr()),
            CreateFontW(-15, 0, 0, 0, FW_NORMAL, 0, 0, 0, DEFAULT_CHARSET, 0, 0,
                        CLEARTYPE_QUALITY, 0, face.as_ptr()),
            CreateFontW(-13, 0, 0, 0, FW_NORMAL, 0, 0, 0, DEFAULT_CHARSET, 0, 0,
                        CLEARTYPE_QUALITY, 0, face.as_ptr()),
            CreateFontW(-12, 0, 0, 0, FW_NORMAL, 0, 0, 0, DEFAULT_CHARSET, 0, 0,
                        CLEARTYPE_QUALITY, 0, face.as_ptr()),
        )
    };

    APP.with(|a| {
        *a.borrow_mut() = Some(App {
            hwnd,
            regions: Vec::new(),
            orientation: Orientation::Vertical,
            mode: ColorMode::Multi,
            flicker: false,
            stripe_visible: false,
            hover: HIT_NONE,
            capturing: false,
            boss_mods: 0,
            boss_vk: 0,
            boss_set: false,
            hint: None,
            bg,
            white,
            border,
            band,
            band_edge,
            canvas_bg,
            f_title,
            f_btn,
            f_body,
            f_small,
            f_help,
            mem_dc: 0,
            mem_bmp: 0,
        });
    });

    // 载入图标（若资源里没有则忽略）
    unsafe {
        let icon = LoadIconW(inst, 1 as *const u16);
        if icon != 0 {
            SendMessageW(hwnd, 0x0080 /* WM_SETICON */, 1 /* ICON_BIG */, icon);
            SendMessageW(hwnd, 0x0080, 0 /* ICON_SMALL */, icon);
        }
    }
    hwnd
}

pub fn show(hwnd: HWND) {
    unsafe {
        ShowWindow(hwnd, SW_SHOW);
        SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
        SetForegroundWindow(hwnd);
        SetFocus(hwnd);
        UpdateWindow(hwnd);
    }
}

pub fn destroy() {
    APP.with(|a| {
        if let Some(app) = a.borrow_mut().take() {
            unsafe {
                DeleteObject(app.bg);
                DeleteObject(app.white);
                DeleteObject(app.border);
                DeleteObject(app.band);
                DeleteObject(app.band_edge);
                DeleteObject(app.canvas_bg);
                if app.f_title != 0 {
                    DeleteObject(app.f_title);
                }
                if app.f_btn != 0 {
                    DeleteObject(app.f_btn);
                }
                if app.f_body != 0 {
                    DeleteObject(app.f_body);
                }
                if app.f_small != 0 {
                    DeleteObject(app.f_small);
                }
                if app.f_help != 0 {
                    DeleteObject(app.f_help);
                }
                if app.mem_dc != 0 {
                    DeleteDC(app.mem_dc);
                    DeleteObject(app.mem_bmp);
                }
            }
        }
    });
}

/// 诊断接口：主窗口句柄
pub fn hwnd() -> HWND {
    APP.with(|a| a.borrow().as_ref().map(|x| x.hwnd).unwrap_or(0))
}

/// 诊断接口：程序化注入一个区域并立即生成条纹（自动化验证用）
pub fn debug_add_region(x: i32, y: i32, w: i32, h: i32) {
    APP.with(|a| {
        let mut guard = match a.try_borrow_mut() {
                    Ok(g) => g,
                    Err(_) => return,
                };
                if let Some(app) = guard.as_mut() {
            let pts = vec![
                POINT { x, y },
                POINT { x: x + w, y },
                POINT { x: x + w, y: y + h },
                POINT { x, y: y + h },
            ];
            app.regions = vec![Region::from_points(pts)];
            app.generate(ColorMode::Multi);
        }
    });
}

/// 诊断接口：切换方向 / 闪烁 / 颜色（自动化验证用）
pub fn debug_set(orientation: Option<Orientation>, flicker: Option<bool>,
                 mode: Option<ColorMode>) {
    APP.with(|a| {
        let mut guard = match a.try_borrow_mut() {
                    Ok(g) => g,
                    Err(_) => return,
                };
                if let Some(app) = guard.as_mut() {
            if let Some(o) = orientation {
                app.orientation = o;
            }
            if let Some(m) = mode {
                app.mode = m;
            }
            if let Some(f) = flicker {
                app.flicker = f;
            }
            if app.stripe_visible && !app.regions.is_empty() {
                layer::show(&app.regions, app.orientation, app.mode, app.flicker);
            }
            app.invalidate();
        }
    });
}

/// 诊断接口：程序化设置老板键（自动化验证用）
pub fn debug_set_hotkey(mods: UINT, vk: u32) {
    APP.with(|a| {
        let mut guard = match a.try_borrow_mut() {
                    Ok(g) => g,
                    Err(_) => return,
                };
                if let Some(app) = guard.as_mut() {
            unsafe {
                if app.boss_set {
                    UnregisterHotKey(app.hwnd, HOTKEY_ID);
                }
                if RegisterHotKey(app.hwnd, HOTKEY_ID, mods, vk) != 0 {
                    app.boss_mods = mods;
                    app.boss_vk = vk;
                    app.boss_set = true;
                }
            }
            app.invalidate();
        }
    });
}

/// 诊断接口：当前状态摘要
pub fn debug_state() -> String {
    APP.with(|a| {
        a.borrow()
            .as_ref()
            .map(|app| {
                format!(
                    "regions={} orientation={} mode={} flicker={} visible={}",
                    app.regions.len(),
                    app.orientation.name(),
                    app.mode.name(),
                    app.flicker,
                    app.stripe_visible
                )
            })
            .unwrap_or_else(|| "无状态".into())
    })
}
