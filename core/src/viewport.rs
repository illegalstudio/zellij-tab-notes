pub fn clamp_scroll(scroll: usize, total_lines: usize, viewport_rows: usize) -> usize {
    scroll.min(total_lines.saturating_sub(viewport_rows))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamps_scroll_to_the_last_full_screen() {
        assert_eq!(clamp_scroll(99, 10, 4), 6);
        assert_eq!(clamp_scroll(2, 10, 4), 2);
    }

    #[test]
    fn does_not_scroll_when_everything_fits() {
        assert_eq!(clamp_scroll(5, 3, 10), 0);
    }
}
