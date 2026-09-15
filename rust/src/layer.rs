//! 条纹覆盖层：覆盖整个虚拟桌面的置顶无边框窗口，色键透明 + 鼠标穿透。
//!
//! 关键点（实测得出）：
//! - `LWA_COLORKEY` 只负责"视觉镂空"，**不负责鼠标穿透**；必须同时加 `WS_EX_TRANSPARENT`，
//!   否则全屏窗口会把整个桌面的点击全部吞掉。
//! - 窗口只创建一次，后续用 rebuild/hide/show 复用，避免反复建窗带来的闪烁与状态归属问题。

#![allow(dead_code)]

use std::cell::RefCell;

use crate::model::{now_ms, Orientation, Region, Rng, Stripe, build_lines, ColorMode, KEY_COLOR,
                   FLICKER_GAP_MAX_MS, FLICKER_GAP_MIN_MS, FLICKER_LINES_MAX, FLICKER_LINES_MIN,
                   FLICKER_MAX_MS, FLICKER_MIN_MS};
use crate::win::*;

pub const LAYER_CLASS: &str = "ScreenStripeLayerW";

/// 闪烁调度节拍（ms）：只用于检查是否有线条需要翻转，代价极低
const TICK_MS: u32 = 40;
const TIMER_ID: usize = 0xA1;

struct Layer {
    hwnd: HWND,
    origin: (i32, i32),
    vw: i32,
    vh: i32,
    orientation: Orientation,
    mode: ColorMode,
    flicker_on: bool,
    /// 每个区域各自的条纹集合
    regions: Vec<Vec<Stripe>>,
    /// 每个区域当前可见线条数（用于保证"区域永不为空"）
    visible_counts: Vec<usize>,
    key_brush: HBRUSH,
    brushes: Vec<(COLORREF, HBRUSH)>,
    rng: Rng,
    /// 是否成功启用了"不被截图捕获"
    capture_excluded: bool,
}

thread_local! {
    static LAYER: RefCell<Option<Layer>> = RefCell::new(None);
}

/// 单条线条在窗口局部坐标下的矩形（自由函数：避免与 regions 的可变借用冲突）
fn stripe_rect(orientation: Orientation, vw: i32, vh: i32, s: &Stripe) -> RECT {
    match orientation {
        Orientation::Vertical => RECT::new(s.pos, 0, s.pos + s.w, vh),
        Orientation::Horizontal => RECT::new(0, s.pos, vw, s.pos + s.w),
    }
}

impl Layer {
    fn brush_of(&self, color: COLORREF) -> HBRUSH {
        for (c, b) in &self.brushes {
            if *c == color {
                return *b;
            }
        }
        0
    }

    /// 重建所有画刷（颜色池随模式变化）
    fn rebuild_brushes(&mut self) {
        unsafe {
            for (_, b) in self.brushes.drain(..) {
                DeleteObject(b);
            }
        }
        let mut colors: Vec<COLORREF> = self.mode.pool().to_vec();
        colors.push(KEY_COLOR);
        unsafe {
            for c in colors {
                if !self.brushes.iter().any(|(cc, _)| *cc == c) {
                    let b = CreateSolidBrush(c);
                    self.brushes.push((c, b));
                }
            }
        }
        self.key_brush = self.brush_of(KEY_COLOR);
    }

    /// 按区域 + 方向重建条纹
    fn rebuild_stripes(&mut self, regions: &[Region]) {
        self.regions.clear();
        for r in regions {
            let (a0, span) = r.axis(self.orientation, self.origin);
            let lines = build_lines(a0, span, self.mode.pool(), &mut self.rng);
            self.regions.push(lines);
        }
        self.visible_counts = self.regions.iter().map(|v| v.len()).collect();
        if self.flicker_on {
            self.pick_flicker_lines();
        }
    }

    /// 每个区域随机挑出"某几条"线条参与闪烁
    fn pick_flicker_lines(&mut self) {
        let now = now_ms();
        for stripes in self.regions.iter_mut() {
            for s in stripes.iter_mut() {
                s.flickers = false;
                s.visible = true;
            }
            let n = stripes.len();
            if n == 0 {
                continue;
            }
            let k = (self
                .rng
                .range(FLICKER_LINES_MIN, FLICKER_LINES_MAX)
                .min(n as u32)) as usize;
            // 随机挑 k 条不重复的索引
            let mut chosen: Vec<usize> = Vec::with_capacity(k);
            while chosen.len() < k {
                let i = self.rng.below(n as u32) as usize;
                if !chosen.contains(&i) {
                    chosen.push(i);
                }
            }
            for i in chosen {
                stripes[i].flickers = true;
                stripes[i].next_flip =
                    now + self.rng.range_ms(FLICKER_GAP_MIN_MS, FLICKER_GAP_MAX_MS);
            }
        }
        self.visible_counts = self.regions.iter().map(|v| v.len()).collect();
    }

    /// 闪烁调度：只翻转到期的线条，并只让那条线所在的窄条失效
    fn tick(&mut self) {
        if !self.flicker_on || self.hwnd == 0 {
            return;
        }
        let now = now_ms();
        let (orientation, vw, vh) = (self.orientation, self.vw, self.vh);
        let mut dirty: Vec<RECT> = Vec::new();
        for (ri, stripes) in self.regions.iter_mut().enumerate() {
            for s in stripes.iter_mut() {
                if !s.flickers || now < s.next_flip {
                    continue;
                }
                if s.visible {
                    // 硬保证：区域至少保留 1 条可见线条
                    if self.visible_counts[ri] <= 1 {
                        s.next_flip =
                            now + self.rng.range_ms(FLICKER_GAP_MIN_MS, FLICKER_GAP_MAX_MS);
                        continue;
                    }
                    s.visible = false;
                    self.visible_counts[ri] -= 1;
                    s.next_flip = now + self.rng.range_ms(FLICKER_MIN_MS, FLICKER_MAX_MS);
                } else {
                    s.visible = true;
                    self.visible_counts[ri] += 1;
                    s.next_flip = now + self.rng.range_ms(FLICKER_GAP_MIN_MS, FLICKER_GAP_MAX_MS);
                }
                dirty.push(stripe_rect(orientation, vw, vh, s));
            }
        }
        for r in dirty {
            unsafe { InvalidateRect(self.hwnd, &r, 0) };
        }
    }

    fn paint(&self, hdc: HDC, rc: &RECT) {
        unsafe {
            FillRect(hdc, rc, self.key_brush);
            for stripes in &self.regions {
                for s in stripes {
                    if !s.visible {
                        continue;
                    }
                    let r = stripe_rect(self.orientation, self.vw, self.vh, s);
                    if !r.intersects(rc) {
                        continue;
                    }
                    let b = self.brush_of(s.color);
                    if b != 0 {
                        FillRect(hdc, &r, b);
                    }
                }
            }
        }
    }
}

unsafe extern "system" fn layer_proc(hwnd: HWND, msg: UINT, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = unsafe { std::mem::zeroed() };
            let hdc = unsafe { BeginPaint(hwnd, &mut ps) };
            LAYER.with(|l| {
                if let Ok(g) = l.try_borrow() {
                    if let Some(layer) = g.as_ref() {
                        if layer.hwnd == hwnd {
                            layer.paint(hdc, &ps.rc_paint);
                        }
                    }
                }
            });
            unsafe { EndPaint(hwnd, &ps) };
            return 0;
        }
        WM_TIMER => {
            if wp == TIMER_ID {
                LAYER.with(|l| {
                    if let Ok(mut g) = l.try_borrow_mut() {
                        if let Some(layer) = g.as_mut() {
                            if layer.hwnd == hwnd {
                                layer.tick();
                            }
                        }
                    }
                });
                return 0;
            }
        }
        WM_ERASEBKGND => return 1, // 禁止系统擦背景，避免闪烁
        _ => {}
    }
    unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
}

/// 注册窗口类并创建覆盖层（初始隐藏）
pub fn create(inst: HINSTANCE) -> HWND {
    let class = wide(LAYER_CLASS);
    let wc = WNDCLASSW {
        style: 0,
        lpfn_wnd_proc: Some(layer_proc),
        cb_cls_extra: 0,
        cb_wnd_extra: 0,
        h_instance: inst,
        h_icon: 0,
        h_cursor: 0,
        hbr_background: 0,
        lpsz_menu_name: std::ptr::null(),
        lpsz_class_name: class.as_ptr(),
    };
    unsafe { RegisterClassW(&wc) };

    let (vx, vy, vw, vh) = virtual_screen();
    let title = wide("屏幕条纹覆盖层");
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE,
            class.as_ptr(),
            title.as_ptr(),
            WS_POPUP,
            vx,
            vy,
            vw,
            vh,
            0,
            0,
            inst,
            std::ptr::null_mut(),
        )
    };
    if hwnd == 0 {
        return 0;
    }
    // 默认不防截图（可被捕获）；由主窗口的勾选项调用 set_capture_excluded 切换
    let capture_excluded = false;
    unsafe {
        SetLayeredWindowAttributes(hwnd, KEY_COLOR, 0, LWA_COLORKEY);
        SetTimer(hwnd, TIMER_ID, TICK_MS, std::ptr::null());
    }

    LAYER.with(|l| {
        *l.borrow_mut() = Some(Layer {
            hwnd,
            origin: (vx, vy),
            vw,
            vh,
            orientation: Orientation::Vertical,
            mode: ColorMode::Multi,
            flicker_on: false,
            regions: Vec::new(),
            visible_counts: Vec::new(),
            key_brush: 0,
            brushes: Vec::new(),
            rng: Rng::new(0x9E37_79B9_7F4A_7C15),
            capture_excluded,
        });
    });
    hwnd
}

/// 按当前参数重建条纹并显示
pub fn show(regions: &[Region], orientation: Orientation, mode: ColorMode, flicker: bool) {
    LAYER.with(|l| {
        if let Some(layer) = l.borrow_mut().as_mut() {
            layer.orientation = orientation;
            layer.mode = mode;
            layer.flicker_on = flicker;
            layer.rebuild_brushes();
            layer.rebuild_stripes(regions);
            let h = layer.hwnd;
            unsafe {
                SetWindowPos(h, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
                ShowWindow(h, SW_SHOW);
                InvalidateRect(h, std::ptr::null(), 0);
            }
        }
    });
}

/// 重新生成条纹（区域/方向/颜色变化时调用）
pub fn rebuild(regions: &[Region], orientation: Orientation, mode: ColorMode) {
    LAYER.with(|l| {
        if let Some(layer) = l.borrow_mut().as_mut() {
            layer.orientation = orientation;
            layer.mode = mode;
            layer.rebuild_brushes();
            layer.rebuild_stripes(regions);
            unsafe { InvalidateRect(layer.hwnd, std::ptr::null(), 0) };
        }
    });
}

/// 隐藏（保留条纹状态）
pub fn hide() {
    LAYER.with(|l| {
        if let Some(layer) = l.borrow().as_ref() {
            if layer.hwnd != 0 {
                unsafe { ShowWindow(layer.hwnd, SW_HIDE) };
            }
        }
    });
}

/// 是否存在可见的覆盖层
pub fn visible() -> bool {
    LAYER.with(|l| {
        l.borrow()
            .as_ref()
            .map(|layer| layer.hwnd != 0 && unsafe { IsWindowVisible(layer.hwnd) } != 0)
            .unwrap_or(false)
    })
}

/// 开关闪烁
pub fn set_flicker(on: bool) {
    LAYER.with(|l| {
        if let Some(layer) = l.borrow_mut().as_mut() {
            if layer.flicker_on == on {
                return;
            }
            layer.flicker_on = on;
            if on {
                layer.pick_flicker_lines();
            } else {
                for stripes in layer.regions.iter_mut() {
                    for s in stripes.iter_mut() {
                        s.flickers = false;
                        s.visible = true;
                    }
                }
                layer.visible_counts = layer.regions.iter().map(|v| v.len()).collect();
            }
            unsafe { InvalidateRect(layer.hwnd, std::ptr::null(), 0) };
        }
    });
}

/// 当前是否开启闪烁
pub fn flicker_on() -> bool {
    LAYER.with(|l| l.borrow().as_ref().map(|x| x.flicker_on).unwrap_or(false))
}

/// 统计当前可见线条总数（诊断用）
pub fn visible_line_count() -> usize {
    LAYER.with(|l| {
        l.borrow()
            .as_ref()
            .map(|layer| layer.visible_counts.iter().sum())
            .unwrap_or(0)
    })
}

/// 线条总数（诊断用）
pub fn total_line_count() -> usize {
    LAYER.with(|l| {
        l.borrow()
            .as_ref()
            .map(|layer| layer.regions.iter().map(|v| v.len()).sum())
            .unwrap_or(0)
    })
}

/// 切换覆盖层的"不被截图捕获"状态
pub fn set_capture_excluded(on: bool) {
    LAYER.with(|l| {
        if let Some(layer) = l.borrow_mut().as_mut() {
            if layer.hwnd == 0 || layer.capture_excluded == on {
                return;
            }
            unsafe {
                // WDA_EXCLUDEFROMCAPTURE：Win10 2004+ 完全不出现在捕获结果里
                // WDA_MONITOR：旧系统兜底，捕获结果里显示为黑块
                let ok = if on {
                    SetWindowDisplayAffinity(layer.hwnd, WDA_EXCLUDEFROMCAPTURE) != 0
                        || SetWindowDisplayAffinity(layer.hwnd, WDA_MONITOR) != 0
                } else {
                    SetWindowDisplayAffinity(layer.hwnd, WDA_NONE) != 0
                };
                if ok {
                    layer.capture_excluded = on;
                }
            }
        }
    });
}

/// 覆盖层窗口句柄（0 = 未创建）
pub fn hwnd() -> HWND {
    LAYER.with(|l| l.borrow().as_ref().map(|x| x.hwnd).unwrap_or(0))
}

/// 把覆盖层抬到 z 序最顶层（用于压过 shell 的开始菜单等窗口）
pub fn raise() {
    LAYER.with(|l| {
        if let Some(layer) = l.borrow().as_ref() {
            if layer.hwnd != 0 {
                unsafe {
                    SetWindowPos(layer.hwnd, HWND_TOP, 0, 0, 0, 0,
                                 SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
                }
            }
        }
    });
}

/// 覆盖层是否已启用"不被截图捕获"
pub fn capture_excluded() -> bool {
    LAYER.with(|l| {
        l.borrow()
            .as_ref()
            .map(|layer| layer.capture_excluded)
            .unwrap_or(false)
    })
}

/// 销毁覆盖层并释放 GDI 资源
pub fn destroy() {
    LAYER.with(|l| {
        if let Some(mut layer) = l.borrow_mut().take() {
            unsafe {
                if layer.hwnd != 0 {
                    KillTimer(layer.hwnd, TIMER_ID);
                    DestroyWindow(layer.hwnd);
                }
                for (_, b) in layer.brushes.drain(..) {
                    DeleteObject(b);
                }
            }
            layer.hwnd = 0;
        }
    });
}
