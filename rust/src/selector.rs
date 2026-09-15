//! 圈选遮罩：全屏半透明置顶窗口，供用户拖动画圈标记条纹覆盖范围。
//!
//! 与覆盖层相反，本窗口 **必须接收鼠标与键盘**，因此绝不能加 `WS_EX_TRANSPARENT`。

#![allow(dead_code)]

use std::cell::RefCell;

use crate::model::{Orientation, Region, MIN_POINT_DIST, MIN_REGION};
use crate::win::*;

pub const SELECTOR_CLASS: &str = "ScreenStripeSelectorW";
const ALPHA: u8 = 115; // 0.45 * 255

struct Selector {
    hwnd: HWND,
    origin: (i32, i32),
    vw: i32,
    vh: i32,
    orientation: Orientation,
    reference: Vec<Region>,
    /// 本次新画的区域（屏幕坐标）
    regions: Vec<Region>,
    /// 正在绘制的轨迹（窗口局部坐标）
    current: Vec<POINT>,
    pressed: bool,
    finished: bool,
    dark: HBRUSH,
    green_pen: HPEN,
    green_thick: HPEN,
    red_dot: HPEN,
    green_dot: HPEN,
    font: HFONT,
    font_bold: HFONT,
}

thread_local! {
    static SEL: RefCell<Option<Selector>> = RefCell::new(None);
}

const HINT: &str = "按住左键拖动画圈 → 标记条纹覆盖范围　｜　可连续圈多个　｜　右键 / 回车 / Esc 结束返回";

impl Selector {
    fn draw(&self, hdc: HDC) {
        unsafe {
            // 全黑底（整窗 45% 不透明 → 桌面被压暗但仍可见）
            let full = RECT::new(0, 0, self.vw, self.vh);
            FillRect(hdc, &full, self.dark);
            SetBkMode(hdc, TRANSPARENT);

            // 外框
            let old = SelectObject(hdc, self.green_thick);
            let oldb = SelectObject(hdc, GetStockObject(NULL_BRUSH));
            Rectangle(hdc, 1, 1, self.vw - 1, self.vh - 1);
            SelectObject(hdc, oldb);
            SelectObject(hdc, old);

            // 提示文字
            let mut r = RECT::new(0, 28, self.vw, 60);
            let oldf = SelectObject(hdc, self.font_bold);
            SetTextColor(hdc, rgb(0, 255, 156));
            let t = wide(HINT);
            DrawTextW(hdc, t.as_ptr(), -1, &mut r, DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
            SelectObject(hdc, oldf);

            // 旧标记：点线轮廓 + 引导线（暗红）
            for reg in &self.reference {
                let pts: Vec<POINT> = reg
                    .points
                    .iter()
                    .map(|p| POINT {
                        x: p.x - self.origin.0,
                        y: p.y - self.origin.1,
                    })
                    .collect();
                let old = SelectObject(hdc, self.red_dot);
                Polyline(hdc, pts.as_ptr(), pts.len() as i32);
                self.draw_guides(hdc, reg, self.red_dot);
                SelectObject(hdc, old);
                self.label(hdc, reg, "原标记", rgb(255, 128, 128));
            }

            // 本次已圈定的区域
            for reg in &self.regions {
                let pts: Vec<POINT> = reg
                    .points
                    .iter()
                    .map(|p| POINT {
                        x: p.x - self.origin.0,
                        y: p.y - self.origin.1,
                    })
                    .collect();
                let old = SelectObject(hdc, self.green_pen);
                Polyline(hdc, pts.as_ptr(), pts.len() as i32);
                self.draw_guides(hdc, reg, self.green_pen);
                SelectObject(hdc, old);
                self.label(hdc, reg, "新标记", rgb(0, 255, 156));
            }

            // 正在绘制的轨迹
            if self.current.len() >= 2 {
                let old = SelectObject(hdc, self.green_pen);
                Polyline(hdc, self.current.as_ptr(), self.current.len() as i32);
                SelectObject(hdc, old);
            }
        }
    }

    /// 按方向画贯穿引导线：竖向画左右边界，横向画上下边界
    fn draw_guides(&self, hdc: HDC, reg: &Region, pen: HPEN) {
        unsafe {
            let old = SelectObject(hdc, pen);
            let (ox, oy) = self.origin;
            match self.orientation {
                Orientation::Vertical => {
                    for x in [reg.x - ox, reg.x + reg.w - ox] {
                        MoveToEx(hdc, x, 0, std::ptr::null_mut());
                        LineTo(hdc, x, self.vh);
                    }
                }
                Orientation::Horizontal => {
                    for y in [reg.y - oy, reg.y + reg.h - oy] {
                        MoveToEx(hdc, 0, y, std::ptr::null_mut());
                        LineTo(hdc, self.vw, y);
                    }
                }
            }
            SelectObject(hdc, old);
        }
    }

    fn label(&self, hdc: HDC, reg: &Region, text: &str, color: COLORREF) {
        unsafe {
            let mut r = RECT::new(
                reg.x - self.origin.0 + 6,
                reg.y - self.origin.1 + 6,
                reg.x - self.origin.0 + 200,
                reg.y - self.origin.1 + 26,
            );
            let oldf = SelectObject(hdc, self.font);
            SetTextColor(hdc, color);
            let t = wide(&format!("{}（{}）", text, self.orientation.name()));
            DrawTextW(hdc, t.as_ptr(), -1, &mut r, DT_LEFT | DT_TOP | DT_SINGLELINE | DT_NOPREFIX);
            SelectObject(hdc, oldf);
        }
    }

    fn add_point(&mut self, x: i32, y: i32) {
        if let Some(last) = self.current.last() {
            if (x - last.x).abs() + (y - last.y).abs() < MIN_POINT_DIST {
                return;
            }
        }
        self.current.push(POINT { x, y });
    }

    fn finish_lasso(&mut self) {
        if self.current.len() >= 3 {
            let (ox, oy) = self.origin;
            let pts: Vec<POINT> = self
                .current
                .iter()
                .map(|p| POINT {
                    x: p.x + ox,
                    y: p.y + oy,
                })
                .collect();
            let reg = Region::from_points(pts);
            if reg.w >= MIN_REGION && reg.h >= MIN_REGION {
                self.regions.push(reg);
            }
        }
        self.current.clear();
        unsafe { InvalidateRect(self.hwnd, std::ptr::null(), 0) };
    }
}

unsafe extern "system" fn sel_proc(hwnd: HWND, msg: UINT, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = unsafe { std::mem::zeroed() };
            let hdc = unsafe { BeginPaint(hwnd, &mut ps) };
            SEL.with(|s| {
                if let Ok(g) = s.try_borrow() {
                    if let Some(sel) = g.as_ref() {
                        if sel.hwnd == hwnd {
                            sel.draw(hdc);
                        }
                    }
                }
            });
            unsafe { EndPaint(hwnd, &ps) };
            return 0;
        }
        WM_LBUTTONDOWN => {
            SEL.with(|s| {
                if let Some(sel) = s.borrow_mut().as_mut() {
                    let (x, y) = cursor_from_lparam(lp);
                    sel.pressed = true;
                    sel.current.clear();
                    sel.add_point(x, y);
                }
            });
            return 0;
        }
        WM_MOUSEMOVE => {
            let mut redraw = false;
            SEL.with(|s| {
                if let Some(sel) = s.borrow_mut().as_mut() {
                    if sel.pressed {
                        let (x, y) = cursor_from_lparam(lp);
                        sel.add_point(x, y);
                        redraw = true;
                    }
                }
            });
            if redraw {
                unsafe { InvalidateRect(hwnd, std::ptr::null(), 0) };
            }
            return 0;
        }
        WM_LBUTTONUP => {
            SEL.with(|s| {
                if let Some(sel) = s.borrow_mut().as_mut() {
                    let (x, y) = cursor_from_lparam(lp);
                    sel.add_point(x, y);
                    sel.pressed = false;
                    sel.finish_lasso();
                }
            });
            return 0;
        }
        WM_RBUTTONDOWN => {
            SEL.with(|s| {
                if let Some(sel) = s.borrow_mut().as_mut() {
                    sel.finished = true;
                    unsafe { ShowWindow(sel.hwnd, SW_HIDE) };
                }
            });
            return 0;
        }
        WM_KEYDOWN => {
            let vk = wp as i32;
            if vk == VK_ESCAPE || vk == VK_RETURN {
                SEL.with(|s| {
                    if let Some(sel) = s.borrow_mut().as_mut() {
                        sel.finished = true;
                        unsafe { ShowWindow(sel.hwnd, SW_HIDE) };
                    }
                });
                return 0;
            }
        }
        WM_ERASEBKGND => return 1,
        _ => {}
    }
    unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
}

pub fn create(inst: HINSTANCE) -> HWND {
    let class = wide(SELECTOR_CLASS);
    let wc = WNDCLASSW {
        style: 0,
        lpfn_wnd_proc: Some(sel_proc),
        cb_cls_extra: 0,
        cb_wnd_extra: 0,
        h_instance: inst,
        h_icon: 0,
        h_cursor: unsafe { LoadCursorW(0, IDC_CROSS) },
        hbr_background: 0,
        lpsz_menu_name: std::ptr::null(),
        lpsz_class_name: class.as_ptr(),
    };
    unsafe { RegisterClassW(&wc) };

    let (vx, vy, vw, vh) = virtual_screen();
    let title = wide("屏幕条纹圈选");
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
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
    let face = wide("Microsoft YaHei UI");
    let (dark, green_pen, green_thick, red_dot, green_dot, font, font_bold) = unsafe {
        (
            CreateSolidBrush(rgb(0, 0, 0)),
            CreatePen(0, 2, rgb(0, 255, 156)),
            CreatePen(0, 2, rgb(0, 255, 156)),
            CreatePen(2, 1, rgb(180, 60, 60)),
            CreatePen(2, 1, rgb(0, 255, 156)),
            CreateFontW(-14, 0, 0, 0, FW_NORMAL, 0, 0, 0, DEFAULT_CHARSET, 0, 0,
                        CLEARTYPE_QUALITY, 0, face.as_ptr()),
            CreateFontW(-16, 0, 0, 0, FW_BOLD, 0, 0, 0, DEFAULT_CHARSET, 0, 0,
                        CLEARTYPE_QUALITY, 0, face.as_ptr()),
        )
    };

    SEL.with(|s| {
        *s.borrow_mut() = Some(Selector {
            hwnd,
            origin: (vx, vy),
            vw,
            vh,
            orientation: Orientation::Vertical,
            reference: Vec::new(),
            regions: Vec::new(),
            current: Vec::new(),
            pressed: false,
            finished: false,
            dark,
            green_pen,
            green_thick,
            red_dot,
            green_dot,
            font,
            font_bold,
        });
    });
    unsafe {
        SetLayeredWindowAttributes(hwnd, 0, ALPHA, LWA_ALPHA);
    }
    hwnd
}

/// 进入圈选模式
pub fn begin(reference: &[Region], orientation: Orientation) {
    SEL.with(|s| {
        if let Some(sel) = s.borrow_mut().as_mut() {
            sel.reference = reference.to_vec();
            sel.orientation = orientation;
            sel.regions.clear();
            sel.current.clear();
            sel.pressed = false;
            sel.finished = false;
            let h = sel.hwnd;
            unsafe {
                SetWindowPos(h, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
                ShowWindow(h, SW_SHOW);
                SetForegroundWindow(h);
                SetFocus(h);
                InvalidateRect(h, std::ptr::null(), 0);
            }
        }
    });
}

/// 阻塞式消息循环，直到用户结束圈选（右键 / 回车 / Esc）
pub fn run_modal() {
    let mut msg: MSG = unsafe { std::mem::zeroed() };
    loop {
        let finished = SEL.with(|s| s.borrow().as_ref().map(|x| x.finished).unwrap_or(true));
        if finished {
            break;
        }
        let r = unsafe { GetMessageW(&mut msg, 0, 0, 0) };
        if r <= 0 {
            break;
        }
        unsafe {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

/// 圈选窗口句柄
pub fn hwnd() -> HWND {
    SEL.with(|s| s.borrow().as_ref().map(|x| x.hwnd).unwrap_or(0))
}

/// 圈选遮罩是否正在显示
pub fn visible() -> bool {
    SEL.with(|s| {
        s.borrow()
            .as_ref()
            .map(|sel| sel.hwnd != 0 && unsafe { IsWindowVisible(sel.hwnd) } != 0)
            .unwrap_or(false)
    })
}

/// 把圈选遮罩抬到最顶层（须在覆盖层之上、主窗口之下）
pub fn raise() {
    SEL.with(|s| {
        if let Some(sel) = s.borrow().as_ref() {
            if sel.hwnd != 0 {
                unsafe {
                    SetWindowPos(sel.hwnd, HWND_TOP, 0, 0, 0, 0,
                                 SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
                }
            }
        }
    });
}

/// 取走本次圈选结果（无新圈则返回空）
pub fn take_result() -> Vec<Region> {
    SEL.with(|s| {
        s.borrow_mut()
            .as_mut()
            .map(|sel| std::mem::take(&mut sel.regions))
            .unwrap_or_default()
    })
}

pub fn destroy() {
    SEL.with(|s| {
        if let Some(mut sel) = s.borrow_mut().take() {
            unsafe {
                if sel.hwnd != 0 {
                    DestroyWindow(sel.hwnd);
                }
                DeleteObject(sel.dark);
                DeleteObject(sel.green_pen);
                DeleteObject(sel.green_thick);
                DeleteObject(sel.red_dot);
                DeleteObject(sel.green_dot);
                DeleteObject(sel.font);
                DeleteObject(sel.font_bold);
            }
            sel.hwnd = 0;
        }
    });
}
