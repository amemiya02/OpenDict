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
    let point = Area {
        x: pointer.0,
        y: pointer.1,
        width: 0,
        height: 0,
    };
    around(point, Gaps::POINTER, size, work_area, scale)
}

/// The top-left corner for a Mini Window of `size` beside the selection, whose bounds on
/// screen are `selection`: just below it, starting where it starts, or just above it, or
/// beside it, whichever keeps the window fully on screen inside `work_area`.
pub fn beside_selection(
    selection: Area,
    size: (u32, u32),
    work_area: Area,
    scale: f64,
) -> (i32, i32) {
    around(selection, Gaps::SELECTION, size, work_area, scale)
}

/// Logical pixels between what the window opens beside and the window.
struct Gaps {
    /// From the foot of the anchor down to the window's top.
    below: f64,
    /// From the window's foot up to the top of the anchor.
    above: f64,
    /// How far left of the anchor the window starts, below or above it.
    indent: f64,
    /// Between the anchor and the window beside it.
    beside: f64,
}

impl Gaps {
    /// The pointer stands somewhere on the line: clear the line, and the pointer itself.
    const POINTER: Gaps = Gaps {
        below: 24.0,
        above: 12.0,
        indent: 12.0,
        beside: 16.0,
    };
    /// The selection's bounds are known: open close to them.
    const SELECTION: Gaps = Gaps {
        below: 6.0,
        above: 6.0,
        indent: 0.0,
        beside: 16.0,
    };
}

fn around(anchor: Area, gaps: Gaps, size: (u32, u32), work_area: Area, scale: f64) -> (i32, i32) {
    let gap = |logical: f64| (logical * scale).round() as i32;
    let (width, height) = (size.0 as i32, size.1 as i32);
    let (left_edge, top_edge) = (work_area.x, work_area.y);
    let right_edge = work_area.x + work_area.width as i32;
    let bottom_edge = work_area.y + work_area.height as i32;
    let anchor_right = anchor.x + anchor.width as i32;
    let anchor_bottom = anchor.y + anchor.height as i32;

    let below = anchor_bottom + gap(gaps.below);
    let above = anchor.y - gap(gaps.above) - height;
    let (left, top) = if below + height <= bottom_edge {
        (anchor.x - gap(gaps.indent), below)
    } else if above >= top_edge {
        (anchor.x - gap(gaps.indent), above)
    } else {
        let right = anchor_right + gap(gaps.beside);
        let left = if right + width <= right_edge {
            right
        } else {
            anchor.x - gap(gaps.beside) - width
        };
        (left, anchor.y + anchor.height as i32 / 2 - height / 2)
    };
    (
        within(left, left_edge, right_edge - width),
        within(top, top_edge, bottom_edge - height),
    )
}

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

    /// A selected word: 120 by 20 pixels.
    fn word(x: i32, y: i32) -> Area {
        Area {
            x,
            y,
            width: 120,
            height: 20,
        }
    }

    #[test]
    fn it_opens_just_below_the_selection_starting_where_it_starts() {
        assert_eq!(
            beside_selection(word(500, 300), SIZE, SCREEN, 1.0),
            (500, 326)
        );
    }

    #[test]
    fn near_the_foot_of_the_screen_it_opens_just_above_the_selection() {
        assert_eq!(
            beside_selection(word(500, 800), SIZE, SCREEN, 1.0),
            (500, 326)
        );
    }

    #[test]
    fn with_room_neither_below_nor_above_it_opens_beside_the_selection() {
        let short = Area {
            height: 700,
            ..SCREEN
        };
        assert_eq!(
            beside_selection(word(600, 340), SIZE, short, 1.0),
            (736, 116)
        );
        assert_eq!(
            beside_selection(word(1700, 340), SIZE, short, 1.0),
            (1276, 116)
        );
    }

    #[test]
    fn beside_a_selection_it_never_leaves_the_screen_sideways() {
        assert_eq!(
            beside_selection(word(1800, 300), SIZE, SCREEN, 1.0),
            (1512, 326)
        );
    }

    #[test]
    fn a_selection_taller_than_the_screen_leaves_the_window_on_screen() {
        let tall = Area {
            x: 100,
            y: -50,
            width: 300,
            height: 2000,
        };
        let (x, y) = beside_selection(tall, SIZE, SCREEN, 1.0);
        assert!((0..=1920 - 408).contains(&x) && (0..=1040 - 468).contains(&y));
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
