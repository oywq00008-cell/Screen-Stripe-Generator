//! 纯逻辑层：随机数、条纹生成、区域模型、参数常量。
//! 与窗口系统完全解耦，移植到其它平台时本文件无需改动。

#![allow(dead_code)]

use crate::win::{POINT, RECT, rgb, COLORREF};
use std::time::Instant;

// ---------------------------------------------------------------- 可调参数

/// 色键（覆盖层中该颜色视为透明）
pub const KEY_COLOR: COLORREF = rgb(1, 2, 3);

/// 五色池：红 / 绿 / 蓝 / 黑 / 白
pub const POOL_MULTI: [COLORREF; 5] = [
    rgb(255, 0, 0),
    rgb(0, 255, 0),
    rgb(0, 0, 255),
    rgb(0, 0, 0),
    rgb(255, 255, 255),
];
pub const POOL_BLACK: [COLORREF; 1] = [rgb(0, 0, 0)];
pub const POOL_WHITE: [COLORREF; 1] = [rgb(255, 255, 255)];

/// 间距池：0 = 两条线完全贴合（权重最高），上限 3px
pub const GAP_POOL: [i32; 8] = [0, 0, 0, 0, 1, 1, 2, 3];

/// 有效圈选的最小跨度（像素）
pub const MIN_REGION: i32 = 2;
/// 手绘轨迹采样间隔，防止点数爆炸
pub const MIN_POINT_DIST: i32 = 5;

/// 闪烁节奏：常亮 1000~5000ms → 消失 100~3000ms → 再常亮…
pub const FLICKER_GAP_MIN_MS: u64 = 1000;
pub const FLICKER_GAP_MAX_MS: u64 = 5000;
pub const FLICKER_MIN_MS: u64 = 100;
pub const FLICKER_MAX_MS: u64 = 3000;
/// 每个区域参与闪烁的线条条数（"某几条"）
pub const FLICKER_LINES_MIN: u32 = 2;
pub const FLICKER_LINES_MAX: u32 = 6;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ColorMode {
    Multi,
    Black,
    White,
}

impl ColorMode {
    pub fn pool(self) -> &'static [COLORREF] {
        match self {
            ColorMode::Multi => &POOL_MULTI,
            ColorMode::Black => &POOL_BLACK,
            ColorMode::White => &POOL_WHITE,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            ColorMode::Multi => "多色",
            ColorMode::Black => "纯黑",
            ColorMode::White => "纯白",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Orientation {
    Vertical,
    Horizontal,
}

impl Orientation {
    pub fn name(self) -> &'static str {
        match self {
            Orientation::Vertical => "竖条纹",
            Orientation::Horizontal => "横条纹",
        }
    }
}

// ---------------------------------------------------------------- 时钟

/// 单调时钟（毫秒），用于闪烁调度
pub fn now_ms() -> u64 {
    use std::sync::OnceLock;
    static BASE: OnceLock<Instant> = OnceLock::new();
    BASE.get_or_init(Instant::now).elapsed().as_millis() as u64
}

// ---------------------------------------------------------------- 随机数

/// xorshift64：自实现而非引入 rand（省体积，且构建无需联网）
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed | 1) // 状态不能为 0
    }
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    /// [0, n)
    pub fn below(&mut self, n: u32) -> u32 {
        if n == 0 {
            return 0;
        }
        (self.next_u64() % n as u64) as u32
    }
    /// [a, b] 闭区间
    pub fn range(&mut self, a: u32, b: u32) -> u32 {
        if b <= a {
            return a;
        }
        a + self.below(b - a + 1)
    }
    pub fn range_ms(&mut self, a: u64, b: u64) -> u64 {
        if b <= a {
            return a;
        }
        a + (self.next_u64() % (b - a + 1))
    }
    pub fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[self.below(items.len() as u32) as usize]
    }
}

// ---------------------------------------------------------------- 条纹

#[derive(Clone, Copy)]
pub struct Stripe {
    /// 沿铺排轴的位置（竖向 = x，横向 = y）
    pub pos: i32,
    /// 线宽（像素）
    pub w: i32,
    pub color: COLORREF,
    /// 闪烁状态：当前是否可见
    pub visible: bool,
    /// 是否参与闪烁
    pub flickers: bool,
    /// 下次状态翻转的时刻（ms，仅 flickers 有效）
    pub next_flip: u64,
}

/// 沿 [a0, a0+span) 铺满细线。
/// 规则：宽 1px、间距取 GAP_POOL、**贴合（间距 0）时沿用前一条的颜色**（同色合并成更宽的实心条）。
pub fn build_lines(a0: i32, span: i32, pool: &[COLORREF], rng: &mut Rng) -> Vec<Stripe> {
    let mut out = Vec::new();
    if span < 1 || pool.is_empty() {
        return out;
    }
    let hi = a0 + span;
    let mut x = a0;
    let mut prev_color: Option<COLORREF> = None;
    let mut prev_gap: i32 = -1;
    let mut guard = 0;
    while x < hi && guard < 20000 {
        guard += 1;
        let mut w = 1;
        if x + w > hi {
            w = hi - x;
        }
        if w < 1 {
            break;
        }
        let color = match (prev_gap == 0, prev_color) {
            (true, Some(c)) => c, // 贴合 → 同色合并
            _ => rng.pick(pool),
        };
        out.push(Stripe {
            pos: x,
            w,
            color,
            visible: true,
            flickers: false,
            next_flip: 0,
        });
        let gap = rng.pick(&GAP_POOL);
        x += w + gap;
        prev_color = Some(color);
        prev_gap = gap;
    }
    out
}

// ---------------------------------------------------------------- 标记区域

#[derive(Clone)]
pub struct Region {
    pub points: Vec<POINT>,
    /// 外接矩形
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Region {
    pub fn from_points(points: Vec<POINT>) -> Self {
        let xs: Vec<i32> = points.iter().map(|p| p.x).collect();
        let ys: Vec<i32> = points.iter().map(|p| p.y).collect();
        let x = *xs.iter().min().unwrap_or(&0);
        let y = *ys.iter().min().unwrap_or(&0);
        let w = *xs.iter().max().unwrap_or(&0) - x;
        let h = *ys.iter().max().unwrap_or(&0) - y;
        Region {
            points,
            x,
            y,
            w,
            h,
        }
    }

    pub fn rect(&self) -> RECT {
        RECT::new(self.x, self.y, self.x + self.w, self.y + self.h)
    }

    /// 该区域的条纹铺排轴起点与跨度（竖向取左右范围，横向取上下范围），
    /// 传入虚拟桌面原点以换算到覆盖层的局部坐标。
    pub fn axis(&self, orientation: Orientation, origin: (i32, i32)) -> (i32, i32) {
        match orientation {
            Orientation::Vertical => (self.x - origin.0, self.w),
            Orientation::Horizontal => (self.y - origin.1, self.h),
        }
    }

    pub fn summary(&self, orientation: Orientation) -> String {
        match orientation {
            Orientation::Vertical => format!(
                "区域：x {}~{}（宽 {}px），竖线贯穿屏幕高度",
                self.x,
                self.x + self.w,
                self.w
            ),
            Orientation::Horizontal => format!(
                "区域：y {}~{}（高 {}px），横线贯穿屏幕宽度",
                self.y,
                self.y + self.h,
                self.h
            ),
        }
    }
}
