//! Where the Mini Window opens: beside the selection, fully on screen.

/// A rectangle on screen, in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Area {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// The top-left corner for a Mini Window of `size` (physical pixels) summoned with the
/// pointer at `pointer`, inside the screen's `work_area`. The pointer stands for the
/// selection, which ends where the reader let go of it: the window goes below the line
/// the pointer is on, or above it, or beside the pointer, whichever keeps it fully on
/// screen. `scale` is the screen's scale factor.
pub fn place(pointer: (i32, i32), size: (u32, u32), work_area: Area, scale: f64) -> (i32, i32) {
    let gap = |logical: f64| (logical * scale).round() as i32;
    let (px, py) = pointer;
    let (width, height) = (size.0 as i32, size.1 as i32);
    let (left_edge, top_edge) = (work_area.x, work_area.y);
    let right_edge = work_area.x + work_area.width as i32;
    let bottom_edge = work_area.y + work_area.height as i32;

    // Clear of the line the pointer is on (and of the pointer itself).
    let below = py + gap(BELOW);
    let above = py - gap(ABOVE) - height;
    let (left, top) = if below + height <= bottom_edge {
        (px - gap(INDENT), below)
    } else if above >= top_edge {
        (px - gap(INDENT), above)
    } else {
        let right = px + gap(BESIDE);
        let left = if right + width <= right_edge {
            right
        } else {
            px - gap(BESIDE) - width
        };
        (left, py - height / 2)
    };
    (
        within(left, left_edge, right_edge - width),
        within(top, top_edge, bottom_edge - height),
    )
}

/// Logical pixels from the pointer down to the window's top when it opens below the line.
const BELOW: f64 = 24.0;
/// Logical pixels from the window's foot up to the pointer when it opens above the line.
const ABOVE: f64 = 12.0;
/// Logical pixels the window starts left of the pointer, below or above the line.
const INDENT: f64 = 12.0;
/// Logical pixels between the pointer and the window when it opens beside the pointer.
const BESIDE: f64 = 16.0;

/// `value` moved into `low..=high`; `low` wins when the range is empty (a window larger
/// than the screen starts at the screen's corner).
fn within(value: i32, low: i32, high: i32) -> i32 {
    value.min(high).max(low)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Area = Area {
        x: 0,
        y: 0,
        width: 1920,
        height: 1040,
    };
    const SIZE: (u32, u32) = (408, 468);

    #[test]
    fn it_opens_below_the_line_the_pointer_is_on() {
        assert_eq!(place((600, 300), SIZE, SCREEN, 1.0), (588, 324));
    }

    #[test]
    fn near_the_foot_of_the_screen_it_opens_above_the_line() {
        assert_eq!(place((600, 800), SIZE, SCREEN, 1.0), (588, 320));
    }

    #[test]
    fn with_room_neither_below_nor_above_it_opens_beside_the_pointer() {
        let short = Area {
            height: 700,
            ..SCREEN
        };
        // Right of the pointer, centred on it.
        assert_eq!(place((600, 350), SIZE, short, 1.0), (616, 116));
        // At the right edge, left of the pointer instead.
        assert_eq!(place((1800, 350), SIZE, short, 1.0), (1376, 116));
    }

    #[test]
    fn it_never_leaves_the_screen_sideways() {
        assert_eq!(place((1900, 300), SIZE, SCREEN, 1.0).0, 1920 - 408);
        assert_eq!(place((2, 300), SIZE, SCREEN, 1.0).0, 0);
    }

    #[test]
    fn it_keeps_to_the_screen_the_pointer_is_on() {
        // A second screen to the left of the first, with a taskbar at its top.
        let left = Area {
            x: -1600,
            y: 40,
            width: 1600,
            height: 860,
        };
        assert_eq!(place((-1590, 45), SIZE, left, 1.0), (-1600, 69));
        assert_eq!(place((-10, 890), SIZE, left, 1.0), (-408, 410));
    }

    #[test]
    fn gaps_grow_with_the_scale_factor() {
        assert_eq!(place((600, 50), (816, 936), SCREEN, 2.0), (576, 98));
    }

    #[test]
    fn a_window_larger_than_the_screen_starts_at_its_corner() {
        let tiny = Area {
            x: 0,
            y: 0,
            width: 300,
            height: 300,
        };
        assert_eq!(place((100, 100), SIZE, tiny, 1.0), (0, 0));
    }
}
