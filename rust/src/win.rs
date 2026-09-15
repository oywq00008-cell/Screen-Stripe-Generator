//! Win32 FFI 层：手写 extern 声明 + 类型别名 + 常用辅助函数。
//!
//! 本文件不引入任何 crate，全部直连系统 DLL，以保证最小体积与离线可构建。

#![allow(non_snake_case, non_camel_case_types, dead_code)]

use std::ffi::c_void;

// ---------------------------------------------------------------- 句柄与基础类型

pub type HWND = isize;
pub type HINSTANCE = isize;
pub type HMENU = isize;
pub type HBRUSH = isize;
pub type HPEN = isize;
pub type HFONT = isize;
pub type HDC = isize;
pub type HBITMAP = isize;
pub type HICON = isize;
pub type HCURSOR = isize;
pub type HGDIOBJ = isize;
pub type HREGION = isize;
pub type BOOL = i32;
pub type UINT = u32;
pub type DWORD = u32;
pub type WPARAM = usize;
pub type LPARAM = isize;
pub type LRESULT = isize;
pub type COLORREF = u32;
pub type ATOM = u16;
pub type WNDPROC = Option<unsafe extern "system" fn(HWND, UINT, WPARAM, LPARAM) -> LRESULT>;

/// 把 #RRGGBB 十六进制转成 COLORREF（0x00BBGGRR）
pub const fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct POINT {
    pub x: i32,
    pub y: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct RECT {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl RECT {
    pub fn new(l: i32, t: i32, r: i32, b: i32) -> Self {
        RECT {
            left: l,
            top: t,
            right: r,
            bottom: b,
        }
    }
    pub fn w(&self) -> i32 {
        self.right - self.left
    }
    pub fn h(&self) -> i32 {
        self.bottom - self.top
    }
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }
    pub fn intersects(&self, o: &RECT) -> bool {
        self.left < o.right && o.left < self.right && self.top < o.bottom && o.top < self.bottom
    }
}

#[repr(C)]
pub struct MSG {
    pub hwnd: HWND,
    pub message: UINT,
    pub wparam: WPARAM,
    pub lparam: LPARAM,
    pub time: DWORD,
    pub pt: POINT,
    pub lprivate: DWORD,
}

#[repr(C)]
pub struct WNDCLASSW {
    pub style: UINT,
    pub lpfn_wnd_proc: WNDPROC,
    pub cb_cls_extra: i32,
    pub cb_wnd_extra: i32,
    pub h_instance: HINSTANCE,
    pub h_icon: HICON,
    pub h_cursor: HCURSOR,
    pub hbr_background: HBRUSH,
    pub lpsz_menu_name: *const u16,
    pub lpsz_class_name: *const u16,
}

#[repr(C)]
pub struct PAINTSTRUCT {
    pub hdc: HDC,
    pub f_erase: BOOL,
    pub rc_paint: RECT,
    pub f_restore: BOOL,
    pub f_inc_update: BOOL,
    pub rgb_reserved: [u8; 32],
}

#[repr(C)]
pub struct TRACKMOUSEEVENT {
    pub cb_size: DWORD,
    pub dw_flags: DWORD,
    pub hwnd_track: HWND,
    pub dw_hover_time: DWORD,
}

// ---------------------------------------------------------------- 函数声明

#[link(name = "kernel32")]
unsafe extern "system" {
    pub fn GetModuleHandleW(name: *const u16) -> HINSTANCE;
}

#[link(name = "user32")]
unsafe extern "system" {
    pub fn RegisterClassW(wc: *const WNDCLASSW) -> ATOM;
    pub fn CreateWindowExW(
        ex_style: DWORD,
        class_name: *const u16,
        window_name: *const u16,
        style: DWORD,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        parent: HWND,
        menu: HMENU,
        inst: HINSTANCE,
        param: *mut c_void,
    ) -> HWND;
    pub fn DefWindowProcW(hwnd: HWND, msg: UINT, wp: WPARAM, lp: LPARAM) -> LRESULT;
    pub fn DestroyWindow(hwnd: HWND) -> BOOL;
    pub fn ShowWindow(hwnd: HWND, cmd: i32) -> BOOL;
    pub fn UpdateWindow(hwnd: HWND) -> BOOL;
    pub fn IsWindowVisible(hwnd: HWND) -> BOOL;
    pub fn GetMessageW(msg: *mut MSG, hwnd: HWND, min: UINT, max: UINT) -> BOOL;
    pub fn TranslateMessage(msg: *const MSG) -> BOOL;
    pub fn DispatchMessageW(msg: *const MSG) -> LRESULT;
    pub fn PostQuitMessage(code: i32);
    pub fn SetLayeredWindowAttributes(hwnd: HWND, key: COLORREF, alpha: u8, flags: DWORD) -> BOOL;
    pub fn GetSystemMetrics(index: i32) -> i32;
    pub fn SetProcessDpiAwarenessContext(ctx: isize) -> BOOL;
    pub fn SetTimer(hwnd: HWND, id: usize, ms: UINT, proc: *const c_void) -> usize;
    pub fn KillTimer(hwnd: HWND, id: usize) -> BOOL;
    pub fn GetAsyncKeyState(vk: i32) -> i16;
    pub fn GetKeyState(vk: i32) -> i16;
    pub fn InvalidateRect(hwnd: HWND, rect: *const RECT, erase: BOOL) -> BOOL;
    pub fn GetClientRect(hwnd: HWND, rect: *mut RECT) -> BOOL;
    pub fn GetWindowRect(hwnd: HWND, rect: *mut RECT) -> BOOL;
    pub fn SetWindowPos(
        hwnd: HWND,
        after: HWND,
        x: i32,
        y: i32,
        cx: i32,
        cy: i32,
        flags: UINT,
    ) -> BOOL;
    pub fn AdjustWindowRectEx(rect: *mut RECT, style: DWORD, menu: BOOL, ex_style: DWORD) -> BOOL;
    pub fn SetCursor(cursor: HCURSOR) -> HCURSOR;
    pub fn LoadCursorW(inst: HINSTANCE, name: *const u16) -> HCURSOR;
    pub fn SetFocus(hwnd: HWND) -> HWND;
    pub fn SetForegroundWindow(hwnd: HWND) -> BOOL;
    pub fn RegisterHotKey(hwnd: HWND, id: i32, modifiers: UINT, vk: UINT) -> BOOL;
    pub fn UnregisterHotKey(hwnd: HWND, id: i32) -> BOOL;
    pub fn TrackMouseEvent(tme: *mut TRACKMOUSEEVENT) -> BOOL;
    pub fn DrawTextW(hdc: HDC, text: *const u16, count: i32, rect: *mut RECT, format: UINT) -> i32;
    pub fn FrameRect(hdc: HDC, rect: *const RECT, brush: HBRUSH) -> BOOL;
    pub fn FindWindowW(class_name: *const u16, window_name: *const u16) -> HWND;
    /// 取 z 序相邻窗口（GW_HWNDPREV = 上方、GW_HWNDNEXT = 下方）
    pub fn GetWindow(hwnd: HWND, cmd: UINT) -> HWND;
    /// 取窗口类名（返回写入的字符数）
    pub fn GetClassNameW(hwnd: HWND, buf: *mut u16, max: i32) -> i32;
    pub fn IsWindow(hwnd: HWND) -> BOOL;
    pub fn SetWindowTextW(hwnd: HWND, text: *const u16) -> BOOL;
    pub fn SendMessageW(hwnd: HWND, msg: UINT, wp: WPARAM, lp: LPARAM) -> LRESULT;
    pub fn LoadIconW(inst: HINSTANCE, name: *const u16) -> HICON;
    pub fn SetActiveWindow(hwnd: HWND) -> HWND;
    pub fn GetFocus() -> HWND;
    pub fn PostMessageW(hwnd: HWND, msg: UINT, wp: WPARAM, lp: LPARAM) -> BOOL;
    /// 设置窗口的显示亲和性：可让窗口不被截图 / 录屏捕获
    pub fn SetWindowDisplayAffinity(hwnd: HWND, affinity: DWORD) -> BOOL;
    /// 查询系统参数（用于取主显示器工作区）
    pub fn SystemParametersInfoW(
        action: UINT,
        param: UINT,
        data: *mut c_void,
        win_ini: UINT,
    ) -> BOOL;
}

#[link(name = "gdi32")]
unsafe extern "system" {
    pub fn CreateSolidBrush(color: COLORREF) -> HBRUSH;
    pub fn CreatePen(style: i32, width: i32, color: COLORREF) -> HPEN;
    pub fn CreateFontW(
        height: i32,
        width: i32,
        escapement: i32,
        orientation: i32,
        weight: i32,
        italic: DWORD,
        underline: DWORD,
        strike_out: DWORD,
        charset: DWORD,
        out_precision: DWORD,
        clip_precision: DWORD,
        quality: DWORD,
        pitch_and_family: DWORD,
        face: *const u16,
    ) -> HFONT;
    pub fn DeleteObject(obj: HGDIOBJ) -> BOOL;
    pub fn SelectObject(hdc: HDC, obj: HGDIOBJ) -> HGDIOBJ;
    pub fn BeginPaint(hwnd: HWND, ps: *mut PAINTSTRUCT) -> HDC;
    pub fn EndPaint(hwnd: HWND, ps: *const PAINTSTRUCT) -> BOOL;
    pub fn FillRect(hdc: HDC, rect: *const RECT, brush: HBRUSH) -> BOOL;
    pub fn RoundRect(hdc: HDC, l: i32, t: i32, r: i32, b: i32, ew: i32, eh: i32) -> BOOL;
    pub fn Rectangle(hdc: HDC, l: i32, t: i32, r: i32, b: i32) -> BOOL;
    pub fn Polyline(hdc: HDC, pts: *const POINT, count: i32) -> BOOL;
    pub fn Polygon(hdc: HDC, pts: *const POINT, count: i32) -> BOOL;
    pub fn MoveToEx(hdc: HDC, x: i32, y: i32, old: *mut POINT) -> BOOL;
    pub fn LineTo(hdc: HDC, x: i32, y: i32) -> BOOL;
    pub fn SetBkMode(hdc: HDC, mode: i32) -> i32;
    pub fn SetTextColor(hdc: HDC, color: COLORREF) -> COLORREF;
    pub fn GetStockObject(index: i32) -> HGDIOBJ;
    pub fn CreateCompatibleDC(hdc: HDC) -> HDC;
    pub fn CreateCompatibleBitmap(hdc: HDC, w: i32, h: i32) -> HBITMAP;
    pub fn DeleteDC(hdc: HDC) -> BOOL;
    pub fn BitBlt(
        dst: HDC,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        src: HDC,
        sx: i32,
        sy: i32,
        rop: DWORD,
    ) -> BOOL;
    pub fn GetDC(hwnd: HWND) -> HDC;
    pub fn ReleaseDC(hwnd: HWND, hdc: HDC) -> i32;
}

// ---------------------------------------------------------------- 常量

// 窗口样式
pub const WS_OVERLAPPED: DWORD = 0x0000_0000;
pub const WS_CAPTION: DWORD = 0x00C0_0000;
pub const WS_SYSMENU: DWORD = 0x0008_0000;
pub const WS_MINIMIZEBOX: DWORD = 0x0002_0000;
pub const WS_POPUP: DWORD = 0x8000_0000;
pub const WS_VISIBLE: DWORD = 0x1000_0000;

pub const WS_EX_LAYERED: DWORD = 0x0008_0000;
pub const WS_EX_TOPMOST: DWORD = 0x0000_0008;
pub const WS_EX_TOOLWINDOW: DWORD = 0x0000_0080;
pub const WS_EX_TRANSPARENT: DWORD = 0x0000_0020;
pub const WS_EX_NOACTIVATE: DWORD = 0x0800_0000;

pub const LWA_COLORKEY: DWORD = 0x0000_0001;
pub const LWA_ALPHA: DWORD = 0x0000_0002;

pub const SW_SHOW: i32 = 5;
pub const SW_HIDE: i32 = 0;
pub const GW_HWNDNEXT: UINT = 2;
pub const GW_HWNDPREV: UINT = 3;
pub const HWND_TOP: HWND = 0; // 置于 z 序最顶层（同组内也可精确重排）
pub const HWND_TOPMOST: HWND = -1;
pub const SWP_NOMOVE: UINT = 0x0002;
pub const SWP_NOSIZE: UINT = 0x0001;
pub const SWP_NOACTIVATE: UINT = 0x0010;
pub const SWP_SHOWWINDOW: UINT = 0x0040;

// GetSystemMetrics
pub const SM_XVIRTUALSCREEN: i32 = 76;
pub const SM_YVIRTUALSCREEN: i32 = 77;
pub const SM_CXVIRTUALSCREEN: i32 = 78;
pub const SM_CYVIRTUALSCREEN: i32 = 79;

// 消息
pub const WM_PAINT: UINT = 0x000F;
pub const WM_DESTROY: UINT = 0x0002;
pub const WM_CLOSE: UINT = 0x0010;
pub const WM_TIMER: UINT = 0x0113;
pub const WM_KEYDOWN: UINT = 0x0100;
pub const WM_KEYUP: UINT = 0x0101;
pub const WM_HOTKEY: UINT = 0x0312;
pub const WM_LBUTTONDOWN: UINT = 0x0201;
pub const WM_LBUTTONUP: UINT = 0x0202;
pub const WM_MOUSEMOVE: UINT = 0x0200;
pub const WM_RBUTTONDOWN: UINT = 0x0204;
pub const WM_MOUSELEAVE: UINT = 0x02A3;
pub const WM_SETCURSOR: UINT = 0x0020;
pub const WM_ERASEBKGND: UINT = 0x0014;
pub const WM_SIZE: UINT = 0x0005;
pub const WM_ACTIVATE: UINT = 0x0006;

// 鼠标跟踪
pub const TME_LEAVE: DWORD = 0x0000_0002;

// 鼠标捕获与光标
pub const HTCLIENT: LRESULT = 1;
pub const IDC_ARROW: *const u16 = 32512 as *const u16;
pub const IDC_HAND: *const u16 = 32649 as *const u16;
pub const IDC_CROSS: *const u16 = 32515 as *const u16;

// 库存 GDI 对象
pub const NULL_BRUSH: i32 = 5;
pub const NULL_PEN: i32 = 8;

// DrawTextW 格式
pub const DT_TOP: UINT = 0x0000;
pub const DT_LEFT: UINT = 0x0000;
pub const DT_CENTER: UINT = 0x0001;
pub const DT_RIGHT: UINT = 0x0002;
pub const DT_VCENTER: UINT = 0x0004;
pub const DT_BOTTOM: UINT = 0x0008;
pub const DT_WORDBREAK: UINT = 0x0010;
pub const DT_SINGLELINE: UINT = 0x0020;
pub const DT_NOPREFIX: UINT = 0x0800;

// SetBkMode
pub const TRANSPARENT: i32 = 1;

// 虚拟键
pub const VK_ESCAPE: i32 = 0x1B;
pub const VK_SHIFT: i32 = 0x10;
pub const VK_CONTROL: i32 = 0x11;
pub const VK_MENU: i32 = 0x12; // Alt
pub const VK_LWIN: i32 = 0x5B;
pub const VK_RETURN: i32 = 0x0D;
pub const VK_SPACE: i32 = 0x20;

// 热键修饰符
pub const MOD_ALT: UINT = 0x0001;
pub const MOD_CONTROL: UINT = 0x0002;
pub const MOD_SHIFT: UINT = 0x0004;
pub const MOD_WIN: UINT = 0x0008;

// 字体
pub const FW_NORMAL: i32 = 400;
pub const FW_BOLD: i32 = 700;
pub const DEFAULT_CHARSET: DWORD = 1;
pub const CLEARTYPE_QUALITY: DWORD = 5;

pub const SRCCOPY: DWORD = 0x00CC_0020;

pub const DPI_PER_MONITOR_AWARE_V2: isize = -4;

/// SPI_GETWORKAREA：取主显示器工作区（屏幕减去任务栏）
pub const SPI_GETWORKAREA: UINT = 0x0030;

/// 把尺寸为 w×h 的窗口摆到主显示器工作区正中；返回 (x, y)。
/// 取工作区而非整屏，避免任务栏较高时窗口被压住；窗口比工作区还大时钳到上边界。
pub fn work_area_center(w: i32, h: i32) -> (i32, i32) {
    unsafe {
        let mut wa = RECT::default();
        if SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut wa as *mut RECT as *mut c_void, 0) != 0
            && wa.w() > 0
            && wa.h() > 0
        {
            let x = wa.left + ((wa.w() - w) / 2).max(0);
            let y = wa.top + ((wa.h() - h) / 2).max(0);
            return (x, y);
        }
        // 兜底：整屏居中
        let sw = GetSystemMetrics(0);
        let sh = GetSystemMetrics(1);
        (((sw - w) / 2).max(0), ((sh - h) / 2).max(0))
    }
}

// 窗口显示亲和性（SetWindowDisplayAffinity）
pub const WDA_NONE: DWORD = 0x0000_0000;
/// 窗口在捕获结果中变为黑色块（Windows 7+ 可用）
pub const WDA_MONITOR: DWORD = 0x0000_0001;
/// 窗口完全不出现在捕获结果中（Windows 10 2004 / build 19041+）
pub const WDA_EXCLUDEFROMCAPTURE: DWORD = 0x0000_0011;

// ---------------------------------------------------------------- 辅助函数

/// 转成以 0 结尾的 UTF-16，供 W 版 API 使用
pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// 虚拟桌面范围
pub fn virtual_screen() -> (i32, i32, i32, i32) {
    unsafe {
        (
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
            GetSystemMetrics(SM_CXVIRTUALSCREEN),
            GetSystemMetrics(SM_CYVIRTUALSCREEN),
        )
    }
}

/// 取 lParam 中的鼠标坐标（低 16 位 x，高 16 位 y，含符号）
pub fn cursor_from_lparam(lp: LPARAM) -> (i32, i32) {
    let x = (lp & 0xFFFF) as u16 as i16 as i32;
    let y = ((lp >> 16) & 0xFFFF) as u16 as i16 as i32;
    (x, y)
}
