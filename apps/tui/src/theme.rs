use std::sync::atomic::{AtomicBool, Ordering};

use ratatui::style::Color;

static LIGHT_MODE: AtomicBool = AtomicBool::new(false);

pub fn toggle() {
    LIGHT_MODE.fetch_xor(true, Ordering::Relaxed);
}

pub fn is_light() -> bool {
    LIGHT_MODE.load(Ordering::Relaxed)
}

// Catppuccin Mocha (dark) / Latte (light) palettes

pub fn base() -> Color {
    if is_light() {
        Color::Rgb(239, 241, 245)
    } else {
        Color::Rgb(30, 30, 46)
    }
}

pub fn mantle() -> Color {
    if is_light() {
        Color::Rgb(230, 233, 239)
    } else {
        Color::Rgb(24, 24, 37)
    }
}

pub fn surface1() -> Color {
    if is_light() {
        Color::Rgb(188, 192, 204)
    } else {
        Color::Rgb(69, 71, 90)
    }
}

pub fn text() -> Color {
    if is_light() {
        Color::Rgb(76, 79, 105)
    } else {
        Color::Rgb(205, 214, 244)
    }
}

pub fn subtext0() -> Color {
    if is_light() {
        Color::Rgb(108, 111, 133)
    } else {
        Color::Rgb(166, 173, 200)
    }
}

pub fn blue() -> Color {
    if is_light() {
        Color::Rgb(30, 102, 245)
    } else {
        Color::Rgb(137, 180, 250)
    }
}

pub fn green() -> Color {
    if is_light() {
        Color::Rgb(64, 160, 43)
    } else {
        Color::Rgb(166, 227, 161)
    }
}

pub fn red() -> Color {
    if is_light() {
        Color::Rgb(210, 15, 57)
    } else {
        Color::Rgb(243, 139, 168)
    }
}

pub fn yellow() -> Color {
    if is_light() {
        Color::Rgb(223, 142, 29)
    } else {
        Color::Rgb(249, 226, 175)
    }
}

pub fn mauve() -> Color {
    if is_light() {
        Color::Rgb(136, 57, 239)
    } else {
        Color::Rgb(203, 166, 247)
    }
}

pub fn peach() -> Color {
    if is_light() {
        Color::Rgb(254, 100, 11)
    } else {
        Color::Rgb(250, 179, 135)
    }
}

pub fn teal() -> Color {
    if is_light() {
        Color::Rgb(23, 146, 153)
    } else {
        Color::Rgb(148, 226, 213)
    }
}

pub fn overlay0() -> Color {
    if is_light() {
        Color::Rgb(156, 160, 176)
    } else {
        Color::Rgb(108, 112, 134)
    }
}

pub fn overlay1() -> Color {
    if is_light() {
        Color::Rgb(140, 143, 161)
    } else {
        Color::Rgb(127, 132, 156)
    }
}
