#![allow(non_upper_case_globals)]

use gearbox::glam::Vec4;

pub const bg0: Vec4 = hex_to_srgb("#191919");
pub const bg1: Vec4 = hex_to_srgb("#1d1d1d");
pub const bg2: Vec4 = hex_to_srgb("#222222");
pub const bg3: Vec4 = hex_to_srgb("#272727");
pub const bg4: Vec4 = hex_to_srgb("#2b2b2b");
pub const bg_statusline1: Vec4 = hex_to_srgb("#282828");
pub const bg_statusline2: Vec4 = hex_to_srgb("#32302f");
pub const bg_statusline3: Vec4 = hex_to_srgb("#504945");
pub const bg_visual_red: Vec4 = hex_to_srgb("#442e2d");
pub const bg_visual_yellow: Vec4 = hex_to_srgb("#473c29");
pub const bg_visual_green: Vec4 = hex_to_srgb("#333e34");
pub const bg_visual_blue: Vec4 = hex_to_srgb("#2e3b3b");
pub const bg_visual_purple: Vec4 = hex_to_srgb("#3c333b");
pub const bg_diff_red: Vec4 = hex_to_srgb("#3c1f1e");
pub const bg_diff_green: Vec4 = hex_to_srgb("#32361a");
pub const bg_diff_blue: Vec4 = hex_to_srgb("#0d3138");
pub const bg_current_word: Vec4 = hex_to_srgb("#32302f");
pub const fg0: Vec4 = hex_to_srgb("#d4be98");
pub const fg1: Vec4 = hex_to_srgb("#ddc7a1");
pub const red: Vec4 = hex_to_srgb("#ea6962");
pub const orange: Vec4 = hex_to_srgb("#e78a4e");
pub const yellow: Vec4 = hex_to_srgb("#d8a657");
pub const green: Vec4 = hex_to_srgb("#a9b665");
pub const aqua: Vec4 = hex_to_srgb("#89b482");
pub const blue: Vec4 = hex_to_srgb("#7daea3");
pub const purple: Vec4 = hex_to_srgb("#d3869b");
pub const bg_red: Vec4 = hex_to_srgb("#ea6962");
pub const bg_green: Vec4 = hex_to_srgb("#a9b665");
pub const bg_yellow: Vec4 = hex_to_srgb("#d8a657");
pub const grey0: Vec4 = hex_to_srgb("#7c6f64");
pub const grey1: Vec4 = hex_to_srgb("#928374");
pub const grey2: Vec4 = hex_to_srgb("#a89984");

/// `"#rrggbb"` as the linear color the UI draws with, like gearbox's `srgb`.
pub const fn hex_to_srgb(string: &str) -> Vec4 {
    let bytes = string.as_bytes();
    let start = if !bytes.is_empty() && bytes[0] == b'#' { 1 } else { 0 };
    assert!(bytes.len() == start + 6, "expected a color like #rrggbb");
    Vec4::new(channel(bytes, start), channel(bytes, start + 2), channel(bytes, start + 4), 1.0)
}

/// The value of one hex digit.
const fn hex_digit(byte: u8) -> u32 {
    match byte {
        b'0'..=b'9' => (byte - b'0') as u32,
        b'a'..=b'f' => (byte - b'a' + 10) as u32,
        b'A'..=b'F' => (byte - b'A' + 10) as u32,
        _ => panic!("not a hex digit"),
    }
}

/// The two digits at `at` as a linear channel from 0 to 1.
const fn channel(bytes: &[u8], at: usize) -> f32 {
    let encoded = (hex_digit(bytes[at]) * 16 + hex_digit(bytes[at + 1])) as f32 / 255.0;
    srgb_to_linear(encoded)
}

/// Undoes the sRGB transfer curve, as gearbox's `srgb` does.
const fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 { c / 12.92 } else { pow_2_4((c + 0.055) / 1.055) }
}

/// `x^2.4` for `x` in `0..=1`, since `powf` can't run in a `const fn`: `x^2 * x^0.4`, where
/// `x^0.4` is the fifth root of `x^2`, found by Newton's method.
const fn pow_2_4(x: f32) -> f32 {
    let square = x * x;
    // y^5 = square; starting above the root, every step lands closer without overshooting
    let mut root = 1.0_f32;
    let mut step = 0;
    while step < 24 {
        let fourth = root * root * root * root;
        root = (4.0 * root + square / fourth) / 5.0;
        step += 1;
    }
    square * root
}

#[cfg(test)]
mod tests {
    use super::*;
    use gearbox::ui::srgb;

    /// Linear channels this close are the same color once drawn.
    const TOLERANCE: f32 = 1e-4;

    fn assert_close(actual: Vec4, expected: Vec4, what: &str) {
        assert!(
            (actual - expected).abs().max_element() < TOLERANCE,
            "{what}: got {actual:?}, gearbox's srgb gives {expected:?}"
        );
    }

    #[test]
    fn black_and_white_are_unchanged() {
        assert_eq!(hex_to_srgb("#000000"), Vec4::new(0.0, 0.0, 0.0, 1.0));
        assert_eq!(hex_to_srgb("#ffffff"), Vec4::ONE);
    }

    #[test]
    fn mid_grey_is_linearized() {
        // sRGB #808080 is about 0.2158 in linear light, not 128 / 255 = 0.502
        assert!((hex_to_srgb("#808080").x - 0.2158).abs() < 1e-3, "got {:?}", hex_to_srgb("#808080"));
    }

    #[test]
    fn every_channel_value_matches_gearbox() {
        for value in 0..=255_u32 {
            let hex = format!("#{value:02x}{value:02x}{value:02x}");
            assert_close(hex_to_srgb(&hex), srgb(value << 16 | value << 8 | value), &hex);
        }
    }

    #[test]
    fn channels_land_in_red_green_blue_order() {
        assert_close(hex_to_srgb("#1d2021"), srgb(0x1d2021), "#1d2021");
        assert_close(hex_to_srgb("#ea6962"), srgb(0xea6962), "#ea6962");
    }

    #[test]
    fn accepts_a_missing_hash_and_uppercase() {
        assert_eq!(hex_to_srgb("1D2021"), hex_to_srgb("#1d2021"));
    }
}
