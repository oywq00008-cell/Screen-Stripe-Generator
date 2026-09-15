//! 启动免责声明弹窗：置顶模态，必须选择「同意」才能继续，选「不同意」则直接退出程序。
//!
//! 与主窗口同样采用整窗自绘 + 命中测试，保持一致的观感，且不引入任何控件类。

#![allow(dead_code)]

use std::cell::RefCell;

use crate::win::*;

pub const DISCLAIMER_CLASS: &str = "ScreenStripeDisclaimerW";

const CW: i32 = 560; // 客户区宽
const CH: i32 = 430; // 客户区高
const X0: i32 = 24; // 内容左边距
const W: i32 = 512; // 内容宽

const C_BG: COLORREF = rgb(255, 255, 255);
const C_TITLE: COLORREF = rgb(31, 35, 40);
const C_BODY: COLORREF = rgb(55, 65, 81);
const C_MUTED: COLORREF = rgb(107, 114, 128);
const C_LINE: COLORREF = rgb(209, 213, 219);
const C_GREEN: COLORREF = rgb(22, 163, 74);
const C_GREEN_H: COLORREF = rgb(21, 128, 61);
const C_GRAY: COLORREF = rgb(107, 114, 128);
const C_GRAY_H: COLORREF = rgb(75, 85, 99);
const C_WHITE: COLORREF = rgb(255, 255, 255);

/// 免责声明正文（段落之间用空行分隔）
const PARAGRAPHS: [&str; 4] = [
    "本软件是一款免费开源软件，遵循 MIT 开源协议；不包含也不提供任何收费内容或付费功能。",
    "本软件仅用于屏幕显示效果的演示与娱乐用途。请勿用于任何违法或不当用途，包括但不限于：\
     伪造设备故障以骗取维修、保修或保险赔付；干扰他人正常工作、学习或造成他人损失；\
     用于任何需要真实性的场合（如报修、举证、鉴定）。",
    "依据 MIT 协议，本软件按「原样」提供，不附带任何明示或默示的担保。\
     如将本软件用于非法或不当用途，由此产生的一切法律责任与后果均由使用者自行承担，\
     作者与贡献者不承担任何责任。",
    "点击「同意并继续」表示你已阅读、理解并接受上述全部条款；点击「不同意」将立即退出本软件。",
];

const HIT_NONE: i32 = 0;
const HIT_AGREE: i32 = 1;
const HIT_DECLINE: i32 = 2;

fn r_agree() -> RECT {
    RECT::new(X0 + W - 200, 354, X0 + W, 396)
}
fn r_decline() -> RECT {
    RECT::new(X0, 354, X0 + 200, 396)
}

struct Disclaimer {
    hwnd: HWND,
    hover: i32,
    /// Some(true)=同意，Some(false)=不同意
    agreed: Option<bool>,
    bg: HBRUSH,
    line: HBRUSH,
    f_title: HFONT,
    f_body: HFONT,
    f_btn: HFONT,
    mem_dc: HDC,
    mem_bmp: HBITMAP,
}

thread_local! {
    static DLG: RefCell<Option<Disclaimer>> = RefCell::new(None);
}

impl Disclaimer {
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

    fn round_fill(&self, hdc: HDC, r: RECT, color: COLORREF) {
        unsafe {
            let b = CreateSolidBrush(color);
            let ob = SelectObject(hdc, b);
            let op = SelectObject(hdc, GetStockObject(NULL_PEN));
            RoundRect(hdc, r.left, r.top, r.right, r.bottom, 14, 14);
            SelectObject(hdc, op);
            SelectObject(hdc, ob);
            DeleteObject(b);
        }
    }

    fn button(&self, hdc: HDC, r: RECT, label: &str, base: COLORREF, hot: COLORREF, on: bool) {
        self.round_fill(hdc, r, if on { hot } else { base });
        self.text(hdc, label, r, self.f_btn, C_WHITE, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
    }

    fn hit(&self, x: i32, y: i32) -> i32 {
        if r_agree().contains(x, y) {
            HIT_AGREE
        } else if r_decline().contains(x, y) {
            HIT_DECLINE
        } else {
            HIT_NONE
        }
    }

    fn paint(&self, hdc: HDC) {
        unsafe { FillRect(hdc, &RECT::new(0, 0, CW, CH), self.bg) };

        // 标题
        self.text(hdc, "免责声明", RECT::new(X0, 22, X0 + W, 48), self.f_title, C_TITLE,
                  DT_LEFT | DT_VCENTER | DT_SINGLELINE);
        // 分隔线
        unsafe { FillRect(hdc, &RECT::new(X0, 54, X0 + W, 55), self.line) };

        // 正文：段落之间留空行
        let body = PARAGRAPHS.join("\n\n");
        self.text(hdc, &body, RECT::new(X0, 66, X0 + W, 344), self.f_body, C_BODY,
                  DT_LEFT | DT_TOP | DT_WORDBREAK);

        // 按钮
        self.button(hdc, r_decline(), "不同意，退出", C_GRAY, C_GRAY_H, self.hover == HIT_DECLINE);
        self.button(hdc, r_agree(), "同意并继续", C_GREEN, C_GREEN_H, self.hover == HIT_AGREE);
    }
}

unsafe extern "system" fn dlg_proc(hwnd: HWND, msg: UINT, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = unsafe { std::mem::zeroed() };
            let hdc = unsafe { BeginPaint(hwnd, &mut ps) };
            DLG.with(|d| {
                let mut guard = match d.try_borrow_mut() {
                    Ok(g) => g,
                    Err(_) => {
                        unsafe { EndPaint(hwnd, &ps) };
                        return;
                    }
                };
                if let Some(dlg) = guard.as_mut() {
                    if dlg.hwnd != hwnd {
                        unsafe { EndPaint(hwnd, &ps) };
                        return;
                    }
                    if dlg.mem_dc == 0 {
                        dlg.mem_dc = unsafe { CreateCompatibleDC(hdc) };
                        dlg.mem_bmp = unsafe { CreateCompatibleBitmap(hdc, CW, CH) };
                        unsafe { SelectObject(dlg.mem_dc, dlg.mem_bmp) };
                    }
                    dlg.paint(dlg.mem_dc);
                    unsafe { BitBlt(hdc, 0, 0, CW, CH, dlg.mem_dc, 0, 0, SRCCOPY) };
                }
            });
            unsafe { EndPaint(hwnd, &ps) };
            return 0;
        }
        WM_ERASEBKGND => return 1,
        WM_MOUSEMOVE => {
            let (x, y) = cursor_from_lparam(lp);
            let mut need = false;
            DLG.with(|d| {
                if let Ok(mut g) = d.try_borrow_mut() {
                    if let Some(dlg) = g.as_mut() {
                        let h = dlg.hit(x, y);
                        if h != dlg.hover {
                            dlg.hover = h;
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
                }
            });
            if need {
                unsafe { InvalidateRect(hwnd, std::ptr::null(), 0) };
            }
            return 0;
        }
        WM_MOUSELEAVE => {
            DLG.with(|d| {
                if let Ok(mut g) = d.try_borrow_mut() {
                    if let Some(dlg) = g.as_mut() {
                        dlg.hover = HIT_NONE;
                    }
                }
            });
            unsafe { InvalidateRect(hwnd, std::ptr::null(), 0) };
            return 0;
        }
        WM_SETCURSOR => {
            let mut hand = false;
            DLG.with(|d| {
                if let Ok(g) = d.try_borrow() {
                    if let Some(dlg) = g.as_ref() {
                        hand = dlg.hover != HIT_NONE;
                    }
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
            DLG.with(|d| {
                if let Ok(mut g) = d.try_borrow_mut() {
                    if let Some(dlg) = g.as_mut() {
                        match dlg.hit(x, y) {
                            HIT_AGREE => {
                                dlg.agreed = Some(true);
                                unsafe { ShowWindow(dlg.hwnd, SW_HIDE) };
                            }
                            HIT_DECLINE => {
                                dlg.agreed = Some(false);
                                unsafe { ShowWindow(dlg.hwnd, SW_HIDE) };
                            }
                            _ => {}
                        }
                    }
                }
            });
            return 0;
        }
        WM_KEYDOWN => {
            let vk = wp as u32;
            // 回车 = 同意；Esc = 不同意（等同于强制退出）
            if vk == VK_RETURN as u32 || vk == VK_ESCAPE as u32 {
                let agree = vk == VK_RETURN as u32;
                DLG.with(|d| {
                    if let Ok(mut g) = d.try_borrow_mut() {
                        if let Some(dlg) = g.as_mut() {
                            dlg.agreed = Some(agree);
                            unsafe { ShowWindow(dlg.hwnd, SW_HIDE) };
                        }
                    }
                });
                return 0;
            }
        }
        // 关闭窗口（Alt+F4 等）视为不同意，避免绕过声明
        WM_CLOSE => {
            DLG.with(|d| {
                if let Ok(mut g) = d.try_borrow_mut() {
                    if let Some(dlg) = g.as_mut() {
                        dlg.agreed = Some(false);
                        unsafe { ShowWindow(dlg.hwnd, SW_HIDE) };
                    }
                }
            });
            return 0;
        }
        _ => {}
    }
    unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
}

/// 创建弹窗（初始隐藏）。返回 0 表示创建失败。
pub fn create(inst: HINSTANCE) -> HWND {
    let class = wide(DISCLAIMER_CLASS);
    let wc = WNDCLASSW {
        style: 0,
        lpfn_wnd_proc: Some(dlg_proc),
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

    // 在主显示器工作区居中
    let (x, y) = work_area_center(CW, CH);

    let title = wide("免责声明");
    // 无标题栏、无系统菜单：没有任何"关闭按钮"可以绕过声明
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            class.as_ptr(),
            title.as_ptr(),
            WS_POPUP,
            x,
            y,
            CW,
            CH,
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
    let (bg, line, f_title, f_body, f_btn) = unsafe {
        (
            CreateSolidBrush(C_BG),
            CreateSolidBrush(C_LINE),
            CreateFontW(-26, 0, 0, 0, FW_BOLD, 0, 0, 0, DEFAULT_CHARSET, 0, 0,
                        CLEARTYPE_QUALITY, 0, face.as_ptr()),
            CreateFontW(-14, 0, 0, 0, FW_NORMAL, 0, 0, 0, DEFAULT_CHARSET, 0, 0,
                        CLEARTYPE_QUALITY, 0, face.as_ptr()),
            CreateFontW(-16, 0, 0, 0, FW_BOLD, 0, 0, 0, DEFAULT_CHARSET, 0, 0,
                        CLEARTYPE_QUALITY, 0, face.as_ptr()),
        )
    };

    DLG.with(|d| {
        *d.borrow_mut() = Some(Disclaimer {
            hwnd,
            hover: HIT_NONE,
            agreed: None,
            bg,
            line,
            f_title,
            f_body,
            f_btn,
            mem_dc: 0,
            mem_bmp: 0,
        });
    });
    hwnd
}

/// 显示弹窗并阻塞等待用户选择；返回 true 表示同意
pub fn ask() -> bool {
    let hwnd = DLG.with(|d| {
        d.borrow().as_ref().map(|x| x.hwnd).unwrap_or(0)
    });
    if hwnd == 0 {
        // 弹窗创建失败时不阻断使用
        return true;
    }
    unsafe {
        ShowWindow(hwnd, SW_SHOW);
        SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
        SetForegroundWindow(hwnd);
        SetFocus(hwnd);
        InvalidateRect(hwnd, std::ptr::null(), 0);
    }

    let mut msg: MSG = unsafe { std::mem::zeroed() };
    loop {
        let done = DLG.with(|d| {
            d.borrow().as_ref().map(|x| x.agreed.is_some()).unwrap_or(true)
        });
        if done {
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
    DLG.with(|d| {
        d.borrow()
            .as_ref()
            .and_then(|x| x.agreed)
            .unwrap_or(false)
    })
}

pub fn destroy() {
    DLG.with(|d| {
        if let Some(dlg) = d.borrow_mut().take() {
            unsafe {
                if dlg.hwnd != 0 {
                    DestroyWindow(dlg.hwnd);
                }
                DeleteObject(dlg.bg);
                DeleteObject(dlg.line);
                DeleteObject(dlg.f_title);
                DeleteObject(dlg.f_body);
                DeleteObject(dlg.f_btn);
                if dlg.mem_dc != 0 {
                    DeleteDC(dlg.mem_dc);
                    DeleteObject(dlg.mem_bmp);
                }
            }
        }
    });
}
