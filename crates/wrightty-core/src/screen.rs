use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line, Point};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::Term;
use alacritty_terminal::vte::ansi::CursorShape as AlacCursorShape;

use wrightty_protocol::types::*;

/// Extract the visible screen as a grid of CellData.
pub fn extract_contents(term: &Term<VoidListener>) -> ScreenGetContentsData {
    let grid = term.grid();
    let num_cols = grid.columns();
    let num_lines = grid.screen_lines();

    let mut cells = Vec::with_capacity(num_lines);

    for line_idx in 0..num_lines {
        let line = Line(line_idx as i32);
        let mut row = Vec::with_capacity(num_cols);

        for col_idx in 0..num_cols {
            let point = Point::new(line, Column(col_idx));
            let cell = &grid[point];

            let c = cell.c;
            let flags = cell.flags;

            let width = if flags.contains(Flags::WIDE_CHAR) {
                2u8
            } else if flags.contains(Flags::WIDE_CHAR_SPACER) {
                0u8
            } else {
                1u8
            };

            // Resolve colors to RGB
            let fg = resolve_color(cell.fg);
            let bg = resolve_color(cell.bg);

            let underline = if flags.contains(Flags::DOUBLE_UNDERLINE) {
                UnderlineStyle::Double
            } else if flags.contains(Flags::UNDERCURL) {
                UnderlineStyle::Curly
            } else if flags.contains(Flags::DOTTED_UNDERLINE) {
                UnderlineStyle::Dotted
            } else if flags.contains(Flags::DASHED_UNDERLINE) {
                UnderlineStyle::Dashed
            } else if flags.contains(Flags::ALL_UNDERLINES) {
                UnderlineStyle::Single
            } else {
                UnderlineStyle::None
            };

            row.push(CellData {
                char: c.to_string(),
                width,
                fg,
                bg,
                attrs: CellAttrs {
                    bold: flags.contains(Flags::BOLD),
                    italic: flags.contains(Flags::ITALIC),
                    underline,
                    underline_color: None, // TODO: extract underline color
                    strikethrough: flags.contains(Flags::STRIKEOUT),
                    dim: flags.contains(Flags::DIM),
                    blink: false, // alacritty_terminal doesn't track blink state on cells
                    reverse: flags.contains(Flags::INVERSE),
                    hidden: flags.contains(Flags::HIDDEN),
                },
                hyperlink: cell.hyperlink().map(|h| h.uri().to_string()),
            });
        }

        cells.push(row);
    }

    let cursor = term.grid().cursor.point;
    let style = term.cursor_style();
    let cursor_state = CursorState {
        row: cursor.line.0 as u32,
        col: cursor.column.0 as u32,
        visible: true,
        shape: match style.shape {
            AlacCursorShape::Block => CursorShape::Block,
            AlacCursorShape::Underline => CursorShape::Underline,
            AlacCursorShape::Beam => CursorShape::Bar,
            _ => CursorShape::Block,
        },
    };

    ScreenGetContentsData {
        rows: num_lines as u32,
        cols: num_cols as u32,
        cursor: cursor_state,
        cells,
        alternate_screen: term.mode().contains(alacritty_terminal::term::TermMode::ALT_SCREEN),
    }
}

/// Extract the visible screen as plain text.
pub fn extract_text(term: &Term<VoidListener>) -> String {
    let grid = term.grid();
    let num_cols = grid.columns();
    let num_lines = grid.screen_lines();
    let mut lines = Vec::with_capacity(num_lines);

    for line_idx in 0..num_lines {
        let line = Line(line_idx as i32);
        let mut row_text = String::with_capacity(num_cols);

        for col_idx in 0..num_cols {
            let point = Point::new(line, Column(col_idx));
            let cell = &grid[point];

            if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                continue;
            }

            row_text.push(cell.c);
        }

        // Trim trailing whitespace
        let trimmed = row_text.trim_end();
        lines.push(trimmed.to_string());
    }

    // Remove trailing empty lines
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }

    lines.join("\n")
}

/// Extract scrollback history lines as plain text.
/// Returns up to `lines` lines starting from `offset` lines before the most recent history line.
pub fn extract_scrollback(
    term: &Term<VoidListener>,
    lines: u32,
    offset: u32,
) -> (Vec<wrightty_protocol::methods::ScrollbackLine>, u32) {
    let grid = term.grid();
    let history = grid.history_size() as u32;
    let num_cols = grid.columns();

    let total_scrollback = history;
    let start = offset;
    let end = (offset + lines).min(history);

    let mut result = Vec::new();
    for i in start..end {
        // Line(-(i+1)) is the (i+1)th most-recent history line
        let line_idx = Line(-((i as i32) + 1));
        let mut row_text = String::with_capacity(num_cols);
        for col_idx in 0..num_cols {
            let point = Point::new(line_idx, Column(col_idx));
            let cell = &grid[point];
            if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                continue;
            }
            row_text.push(cell.c);
        }
        let text = row_text.trim_end().to_string();
        result.push(wrightty_protocol::methods::ScrollbackLine {
            text,
            line_number: -((i as i32) + 1),
        });
    }

    (result, total_scrollback)
}

/// Data returned by extract_contents (before serialization to protocol type).
pub struct ScreenGetContentsData {
    pub rows: u32,
    pub cols: u32,
    pub cursor: CursorState,
    pub cells: Vec<Vec<CellData>>,
    pub alternate_screen: bool,
}

/// Resolve an alacritty color to RGB.
/// For now, use a simple default palette. Full palette support comes later.
fn resolve_color(color: alacritty_terminal::vte::ansi::Color) -> Rgb {
    use alacritty_terminal::vte::ansi::Color;
    use alacritty_terminal::vte::ansi::NamedColor;

    match color {
        Color::Spec(rgb) => Rgb {
            r: rgb.r,
            g: rgb.g,
            b: rgb.b,
        },
        Color::Named(named) => {
            // Default xterm colors
            match named {
                NamedColor::Black => Rgb { r: 0, g: 0, b: 0 },
                NamedColor::Red => Rgb {
                    r: 205,
                    g: 0,
                    b: 0,
                },
                NamedColor::Green => Rgb {
                    r: 0,
                    g: 205,
                    b: 0,
                },
                NamedColor::Yellow => Rgb {
                    r: 205,
                    g: 205,
                    b: 0,
                },
                NamedColor::Blue => Rgb {
                    r: 0,
                    g: 0,
                    b: 238,
                },
                NamedColor::Magenta => Rgb {
                    r: 205,
                    g: 0,
                    b: 205,
                },
                NamedColor::Cyan => Rgb {
                    r: 0,
                    g: 205,
                    b: 205,
                },
                NamedColor::White => Rgb {
                    r: 229,
                    g: 229,
                    b: 229,
                },
                NamedColor::BrightBlack => Rgb {
                    r: 127,
                    g: 127,
                    b: 127,
                },
                NamedColor::BrightRed => Rgb {
                    r: 255,
                    g: 0,
                    b: 0,
                },
                NamedColor::BrightGreen => Rgb {
                    r: 0,
                    g: 255,
                    b: 0,
                },
                NamedColor::BrightYellow => Rgb {
                    r: 255,
                    g: 255,
                    b: 0,
                },
                NamedColor::BrightBlue => Rgb {
                    r: 92,
                    g: 92,
                    b: 255,
                },
                NamedColor::BrightMagenta => Rgb {
                    r: 255,
                    g: 0,
                    b: 255,
                },
                NamedColor::BrightCyan => Rgb {
                    r: 0,
                    g: 255,
                    b: 255,
                },
                NamedColor::BrightWhite => Rgb {
                    r: 255,
                    g: 255,
                    b: 255,
                },
                NamedColor::Foreground => Rgb {
                    r: 255,
                    g: 255,
                    b: 255,
                },
                NamedColor::Background => Rgb { r: 0, g: 0, b: 0 },
                _ => Rgb {
                    r: 200,
                    g: 200,
                    b: 200,
                },
            }
        }
        Color::Indexed(idx) => {
            // 256-color palette
            static ANSI_COLORS: [(u8, u8, u8); 16] = [
                (0, 0, 0),       // 0 black
                (205, 0, 0),     // 1 red
                (0, 205, 0),     // 2 green
                (205, 205, 0),   // 3 yellow
                (0, 0, 238),     // 4 blue
                (205, 0, 205),   // 5 magenta
                (0, 205, 205),   // 6 cyan
                (229, 229, 229), // 7 white
                (127, 127, 127), // 8 bright black
                (255, 0, 0),     // 9 bright red
                (0, 255, 0),     // 10 bright green
                (255, 255, 0),   // 11 bright yellow
                (92, 92, 255),   // 12 bright blue
                (255, 0, 255),   // 13 bright magenta
                (0, 255, 255),   // 14 bright cyan
                (255, 255, 255), // 15 bright white
            ];

            if (idx as usize) < 16 {
                let (r, g, b) = ANSI_COLORS[idx as usize];
                Rgb { r, g, b }
            } else if idx < 232 {
                // 6x6x6 color cube
                let i = idx - 16;
                let r = (i / 36) * 51;
                let g = ((i / 6) % 6) * 51;
                let b = (i % 6) * 51;
                Rgb { r, g, b }
            } else {
                // Grayscale ramp
                let v = 8 + (idx - 232) * 10;
                Rgb { r: v, g: v, b: v }
            }
        }
    }
}
