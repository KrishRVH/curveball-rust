//! Const keyframe tables extracted from the SWF sprite timelines.
//!
//! Pip flash: sprites 59/75 — each label (`UR`/`UL`/`BL`/`BR`/`C`) runs a
//! 10-frame segment of per-frame alpha cxforms on the five pip overlays.
//! Banner: sprite 62 frames 10–70 — 61 frames of (text y offset, alpha).
//! All alpha values are Flash cxform multipliers out of 256.

/// Alpha of the pip that was hit, per flash tick (frame 1 of each segment
/// carries an identity cxform = 256).
pub const HIT_PIP_ALPHA: [u16; 10] = [256, 238, 219, 201, 182, 164, 145, 127, 108, 90];

/// Alpha of the other four pips during any flash.
pub const OTHER_PIP_ALPHA: [u16; 10] = [192, 181, 169, 158, 147, 135, 124, 113, 101, 90];

/// Center-hit color ramp on the C pip: rgb multiplier (of 256) per tick…
pub const C_PIP_MULT: [u16; 10] = [0, 28, 57, 85, 114, 142, 171, 199, 228, 256];

/// …plus an additive red term, so the pip ramps red → white while fading:
/// `rgb = (255·mult/256 + add_red, 255·mult/256, 255·mult/256)`.
pub const C_PIP_ADD_RED: [u16; 10] = [255, 227, 198, 170, 142, 113, 85, 57, 28, 0];

/// Banner keyframes (text y offset from the (175, 206) anchor, alpha of 256):
/// in 16 frames, dip 15, rise 16, out 14 — 61 ticks total. The y values are
/// the exact placement matrices (not perfectly linear in the original).
pub const BANNER_FRAMES: [(f32, u16); 61] = [
    // in: rise from the bar while fading in
    (2.75, 0),
    (1.4, 17),
    (0.1, 34),
    (-1.25, 51),
    (-2.6, 68),
    (-3.9, 85),
    (-5.25, 102),
    (-6.6, 119),
    (-7.9, 137),
    (-9.25, 154),
    (-10.6, 171),
    (-11.9, 188),
    (-13.25, 205),
    (-14.6, 222),
    (-15.9, 239),
    (-17.25, 256),
    // dip: hold position, breathe down
    (-17.25, 244),
    (-17.25, 232),
    (-17.25, 220),
    (-17.25, 208),
    (-17.25, 196),
    (-17.25, 184),
    (-17.25, 172),
    (-17.25, 161),
    (-17.25, 149),
    (-17.25, 137),
    (-17.25, 125),
    (-17.25, 113),
    (-17.25, 101),
    (-17.25, 89),
    (-17.25, 77),
    // rise: breathe back up
    (-17.25, 88),
    (-17.25, 99),
    (-17.25, 111),
    (-17.25, 122),
    (-17.25, 133),
    (-17.25, 144),
    (-17.25, 155),
    (-17.25, 167),
    (-17.25, 178),
    (-17.25, 189),
    (-17.25, 200),
    (-17.25, 211),
    (-17.25, 222),
    (-17.25, 234),
    (-17.25, 245),
    (-17.25, 256),
    // out: sink back while fading
    (-15.8, 238),
    (-14.4, 219),
    (-12.95, 201),
    (-11.55, 183),
    (-10.1, 165),
    (-8.7, 146),
    (-7.25, 128),
    (-5.8, 110),
    (-4.4, 91),
    (-2.95, 73),
    (-1.55, 55),
    (-0.1, 37),
    (1.3, 18),
    (2.75, 0),
];

#[cfg(test)]
mod tests {
    use curveball::app::{BANNER_TICKS, PIP_FLASH_TICKS};

    use super::*;

    /// The app's animation clocks index these tables; the lengths must agree.
    #[test]
    fn table_lengths_match_animation_clocks() {
        assert_eq!(BANNER_FRAMES.len(), BANNER_TICKS as usize);
        assert_eq!(HIT_PIP_ALPHA.len(), PIP_FLASH_TICKS as usize);
        assert_eq!(OTHER_PIP_ALPHA.len(), PIP_FLASH_TICKS as usize);
        assert_eq!(C_PIP_MULT.len(), PIP_FLASH_TICKS as usize);
        assert_eq!(C_PIP_ADD_RED.len(), PIP_FLASH_TICKS as usize);
    }
}
