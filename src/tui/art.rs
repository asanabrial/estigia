use ratatui::style::Color;

/// Tantan, the kitten from the README cover, one braille row per entry. Blanks are U+2800 and never a
/// space, and U+2804 is never used: some terminal fonts draw it wider than its neighbours and shear the
/// picture.
pub(super) const TANTAN: &[&str] = &[
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣀⣤⣴⠖⣻⣿⠛⣻⡿⠳⠶⢶⣤⣄⡀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣠⣴⠟⢩⡟⢁⣼⡟⠁⣴⠏⣠⠶⠶⠶⣄⠉⠛⢷⣄⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⣀⣤⡴⠶⠶⠞⠛⠛⠛⠻⠏⢠⡿⠀⣼⡿⠀⣼⡏⠰⡴⠛⠙⠳⣮⡻⣆⠀⠉⢳⣄⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠻⣖⠒⠶⣤⣄⡀⠀⠀⠀⠀⣸⠇⢰⣿⠃⢰⡟⠀⣿⠃⠀⠀⣷⣾⣷⡈⢧⠀⠀⠙⣧⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠈⠳⣄⡀⠉⣿⣷⡄⠀⠀⣿⠀⣼⡟⠀⣿⠁⠀⠈⠀⠀⠀⠿⣿⡿⡇⢸⠀⠀⠀⠘⣧⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠈⠛⣦⡛⠋⠀⠀⠸⡇⠀⣿⠃⢠⡟⠀⠀⠀⠀⠀⠀⠀⠈⠃⠀⠈⠀⠀⠀⠀⣹⣧⣀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣾⠁⠀⠀⠀⠀⢿⡀⢿⠀⣸⠁⠀⠀⡀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣿⢸⣟⢷⣦⡀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢿⠀⠀⠀⠀⢰⡈⠧⠸⠀⠃⢰⡄⠀⠟⠀⠀⠀⠀⠀⠀⠀⠀⣠⡾⠀⢀⡏⢸⣿⠀⠻⠙⣦⡀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠘⣧⣄⣀⣤⣤⡀⠀⠀⠀⠀⠀⣠⣴⣶⠶⣄⠀⢀⣀⣀⣤⠾⠋⠁⠀⣼⠁⣾⡟⠀⠀⠀⠈⠳⣄⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠹⡄⢸⡀⣯⣿⣆⠀⠀⠀⢠⡏⢾⣿⡇⣸⠀⠈⠉⠉⢁⣠⡾⠀⣰⠇⣼⣿⠃⠀⠀⢨⣷⠀⠙⣧⡀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠙⡆⣙⠮⠿⢾⡀⠀⠀⠀⠟⠺⠿⠞⣫⡀⠀⠈⠉⠉⠁⢀⠾⣃⣾⡿⠃⣠⠀⠀⣼⡿⠀⠀⣌⢻⡄⠀⠀⠀⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠠⠖⠒⠒⠀⠉⠀⠀⠀⣀⣀⣀⡀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠈⠿⠋⢀⣴⠟⠀⢠⣿⠃⠀⠀⣿⠀⢻⣆⠀⠀⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢀⣤⡼⠥⠤⢤⣤⣀⠈⡻⡋⠀⠀⢀⡠⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢀⣴⣛⠁⠀⠰⠛⠁⠀⠀⣰⡿⠀⠸⣿⢧⡀⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢀⡴⠋⠀⠀⠀⠀⠀⠀⠀⠱⡄⠈⠑⠊⠁⠀⠀⠀⠀⠀⠀⠀⢀⡴⠞⠋⠉⠉⠉⠙⠻⣏⠛⢿⣝⠳⢶⡄⠀⠀⣿⡀⢻⡄⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⣀⣼⠁⡄⠀⠀⡀⠀⠀⢀⠀⢀⡗⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢠⠏⢀⠀⠀⢀⠀⠀⠀⠀⠘⣷⠀⢹⣦⠈⢿⡆⢀⣿⠃⢀⢻⡄⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⢰⡏⠈⠣⣷⣀⣀⣧⣀⡤⠟⠙⠛⠛⠛⠛⠛⠛⠛⠲⠶⠶⠶⠶⢾⡀⡏⠀⠀⡏⠀⠀⣠⣧⣤⣿⣇⣀⣿⣀⣸⣇⣸⣟⠀⣸⡌⣿⡄⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⢸⡇⠀⣿⠉⠉⠛⠛⠛⠛⠓⠒⠒⠒⠶⠶⠶⠶⠦⠤⠤⠤⣤⣤⣤⣽⣿⣦⣤⣿⣶⣞⣋⣀⡀⠀⠀⠉⠉⠉⠉⠉⠉⠉⠉⠛⠛⠛⠛⠛⢿⣦",
    "⠀⠀⠀⠀⠀⠀⠀⠀⢸⡇⠀⡿⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠈⠉⠉⠉⠉⠉⠉⠙⠛⠛⠛⠛⠒⠒⠒⠲⠶⢶⠜⠛⣾⣿",
    "⠀⠀⠀⠀⠀⠀⠀⠀⢸⡇⠀⡇⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢸⠀⠀⣿⣿",
    "⠀⠀⠀⠀⠀⠀⠀⠀⢸⡇⠀⡇⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢀⡀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢸⠀⠀⣿⡟",
    "⠀⠀⠀⠀⠀⠀⠀⠀⢸⡇⠀⡇⠀⠀⠀⠀⠀⢀⡴⠶⢦⡀⢀⣤⣤⣤⡀⢀⣸⣇⣀⠀⣈⡁⠀⠀⠀⠀⠀⠀⠶⣶⠶⠀⢀⣤⣤⣤⡀⠀⠀⠀⠀⠀⠀⢸⠀⠀⣿⡇",
    "⠀⠀⠀⠀⠀⠀⠀⠀⢸⡇⠀⡇⠀⠀⠀⠀⠀⢸⡟⠛⢛⡃⠘⠷⠶⢶⡄⠈⢹⡏⠉⠀⢹⡇⠀⣾⠛⠛⣿⠀⠀⣿⠀⠀⣿⠀⠀⠀⣿⠀⠀⠀⠀⠀⠀⢸⠀⠀⣿⡇",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⡇⠀⡇⠀⠀⠀⠀⠀⠀⠙⠛⠋⠀⠐⠶⠶⠾⠃⠀⠸⣧⣤⠀⣼⣧⠀⠻⠶⠶⣿⠀⠀⣿⠀⠀⣿⠛⠛⠛⣿⠀⠀⠀⠀⠀⠀⣸⠀⠀⣿⡇",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⡇⠀⡇⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣀⣀⣀⡿⠀⠛⠛⠛⠀⠿⠀⠀⠀⠿⠀⠀⠀⠀⠀⠀⣿⠀⢀⣿⡇",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⡇⠀⡇⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠉⠉⠉⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣿⠀⢸⣿⡇",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⡇⠀⠷⣤⣤⣀⣀⣀⣀⣀⣀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣿⠀⢸⣿⠀",
    "⠀⠀⠀⠀⠀⠀⠀⢀⣴⠷⠤⠤⠤⠤⢤⣤⣄⣀⣉⣉⣉⣉⠉⠉⠛⠛⠒⠒⠒⠒⠲⠶⠤⠤⠤⠤⢤⣤⣄⣀⣀⣀⣀⣀⡀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣿⠀⢸⣿⠀",
    "⠀⠀⠀⠀⠀⠀⣠⠞⠁⣠⠤⠖⠒⣲⡖⠲⣤⡀⠀⠀⠈⠉⠉⠉⠉⠉⠙⠛⠒⠒⠒⠒⠶⠶⠤⠤⠤⠤⣤⣤⣀⣀⣉⣉⣉⡉⠉⠉⠙⠛⠓⠒⠒⠒⠶⠟⠀⢸⣿⠀",
    "⠀⠀⠀⠀⣠⠞⠁⢠⢾⠳⠶⠀⠀⠉⣡⣄⢹⢹⡀⠀⠀⢠⠞⠛⠒⡶⠀⣠⠶⠤⢤⡄⠀⣠⣀⣀⣀⠀⠀⠀⠀⠀⠀⠀⠉⠉⠉⠉⠉⠉⠛⠓⠒⠒⠒⡶⣤⣸⣿⠀",
    "⠀⠀⣠⡾⠃⠀⠀⢸⡾⣄⣠⣶⠖⢀⣈⡭⣞⡽⠁⠀⣀⣙⠛⠒⠚⠁⠐⠧⠤⠤⠞⠀⢰⣃⣀⣀⡞⠁⢀⡞⠉⠉⣹⠇⢀⡴⠒⠒⢲⠀⠀⠀⠀⢀⡞⠁⣸⣿⡟⠀",
    "⢠⡾⠋⠀⠀⠀⠀⠀⠙⠳⠯⠭⠭⠭⠶⠞⠋⠀⠀⣴⣃⠈⣩⠟⢀⡴⠛⠒⣲⠆⢀⡴⠤⠤⣤⠀⠀⢀⣈⣉⣉⠛⠁⠀⠘⠒⠶⠶⠋⠀⠀⠀⢠⠏⢀⣼⣿⠏⠀⠀",
    "⣿⠙⠓⠒⠒⠲⠶⠤⠤⠤⣤⣀⣀⣀⣀⡀⠀⠀⠀⠀⠉⠉⠁⠀⠘⠓⠒⠒⠃⠀⠾⠤⠤⠴⠃⠀⢰⣋⣀⣀⡼⠁⣠⠏⠉⠙⣲⠀⠀⠀⠀⣰⠋⢀⣾⡿⠁⠀⠀⠀",
    "⢿⣶⣤⣤⣤⣤⣀⣀⣀⣀⠀⠀⠀⠀⠈⠉⠉⠉⠙⠛⠒⠒⠒⠶⠤⠤⠤⣤⣄⣀⣀⣀⡀⠀⠀⠀⠀⠀⠉⠉⠀⠀⠉⠛⠒⠚⠁⠀⠀⢀⡼⠁⣠⣿⠟⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠉⠉⠉⠉⠛⠛⠛⠛⠿⠿⠿⢶⣶⣶⣦⣤⣤⣤⣄⣀⣀⣀⣀⠀⠀⠀⠀⠀⠉⠉⠉⠉⠙⠛⠒⠒⠒⠶⠤⠤⠤⣤⣄⣀⣀⣀⡞⠁⣴⡿⠋⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠉⠉⠉⠉⠙⠛⠛⠛⠛⠿⠿⠿⠶⣶⣶⣦⣤⣤⣤⣄⣀⣀⣀⣀⠀⠀⠀⠀⠀⠀⢹⢀⣼⡟⠁⠀⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠉⠉⠉⠉⠉⠛⠛⠛⠛⠻⠿⠿⠷⢶⣾⡿⠋⠀⠀⠀⠀⠀⠀⠀⠀⠀",
];

const GRADIENT: [(u8, u8, u8); 5] = [
    (125, 220, 235),
    (90, 190, 230),
    (70, 150, 220),
    (60, 110, 200),
    (55, 80, 170),
];

pub(super) fn width(art: &[&str]) -> u16 {
    u16::try_from(art.iter().map(|row| row.chars().count()).max().unwrap_or(0)).unwrap_or(u16::MAX)
}

pub(super) fn height(art: &[&str]) -> u16 {
    u16::try_from(art.len()).unwrap_or(u16::MAX)
}

pub(super) fn band(row: usize, rows: usize) -> Color {
    let stop = (row * GRADIENT.len())
        .checked_div(rows)
        .map_or(0, |stop| stop.min(GRADIENT.len() - 1));
    let (red, green, blue) = GRADIENT[stop];
    Color::Rgb(red, green, blue)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blank(row: &str) -> bool {
        row.chars().all(|c| c == '\u{2800}')
    }

    #[test]
    fn the_drawing_is_a_rectangle_of_braille_and_nothing_else() {
        let wide = width(TANTAN);
        assert!(!TANTAN.is_empty());
        for row in TANTAN {
            assert_eq!(row.chars().count(), usize::from(wide));
            assert!(row.chars().all(|c| ('\u{2800}'..='\u{28FF}').contains(&c)));
        }
    }

    #[test]
    fn no_cell_is_a_space_or_the_dot_that_fonts_draw_wide() {
        for row in TANTAN {
            assert!(!row.contains(' ') && !row.contains('\u{2804}'));
        }
    }

    #[test]
    fn the_drawing_has_no_blank_margin_above_or_below() {
        assert!(!blank(TANTAN[0]) && !blank(TANTAN[TANTAN.len() - 1]));
    }

    #[test]
    fn the_drawing_touches_both_side_edges_of_the_rectangle() {
        let edge = |at: usize| {
            TANTAN
                .iter()
                .any(|row| row.chars().nth(at) != Some('\u{2800}'))
        };
        assert!(edge(0) && edge(usize::from(width(TANTAN)) - 1));
    }

    #[test]
    fn an_empty_drawing_bands_without_dividing_by_zero() {
        assert_eq!(band(0, 0), Color::Rgb(125, 220, 235));
    }

    #[test]
    fn the_gradient_runs_from_the_first_stop_to_the_last() {
        let rows = TANTAN.len();
        assert_eq!(band(0, rows), Color::Rgb(125, 220, 235));
        assert_eq!(band(rows - 1, rows), Color::Rgb(55, 80, 170));
        let distinct: std::collections::HashSet<_> = (0..rows)
            .map(|row| format!("{:?}", band(row, rows)))
            .collect();
        assert_eq!(distinct.len(), GRADIENT.len());
    }
}
