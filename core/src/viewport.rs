/// Wraps note content to `cols` columns, word-aware, hard-splitting words that are
/// longer than a line. Blank lines are preserved so markdown keeps its shape.
pub fn wrap(content: &str, cols: usize) -> Vec<String> {
    if cols == 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for line in content.lines() {
        let mut current = String::new();
        for word in line.split_whitespace() {
            let mut word = word;
            while word.chars().count() > cols {
                if !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
                let split_at = word
                    .char_indices()
                    .nth(cols)
                    .map(|(index, _)| index)
                    .unwrap_or(word.len());
                let (head, tail) = word.split_at(split_at);
                out.push(head.to_string());
                word = tail;
            }
            if word.is_empty() {
                continue;
            }
            if current.is_empty() {
                current = word.to_string();
            } else if current.chars().count() + 1 + word.chars().count() <= cols {
                current.push(' ');
                current.push_str(word);
            } else {
                out.push(std::mem::take(&mut current));
                current = word.to_string();
            }
        }
        out.push(current);
    }
    out
}

pub fn clamp_scroll(scroll: usize, total_lines: usize, viewport_rows: usize) -> usize {
    scroll.min(total_lines.saturating_sub(viewport_rows))
}

pub fn is_heading(line: &str) -> bool {
    line.trim_start().starts_with('#')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_lines_pass_through_unchanged() {
        assert_eq!(wrap("one\ntwo", 20), vec!["one", "two"]);
    }

    #[test]
    fn preserves_blank_lines() {
        assert_eq!(wrap("a\n\nb", 20), vec!["a", "", "b"]);
    }

    #[test]
    fn wraps_on_word_boundaries() {
        assert_eq!(wrap("alpha beta gamma", 11), vec!["alpha beta", "gamma"]);
    }

    #[test]
    fn hard_splits_a_word_longer_than_the_line() {
        assert_eq!(wrap("aaaaaaa", 3), vec!["aaa", "aaa", "a"]);
    }

    #[test]
    fn a_zero_width_viewport_produces_nothing() {
        assert_eq!(wrap("anything", 0), Vec::<String>::new());
    }

    #[test]
    fn clamps_scroll_to_the_last_full_screen() {
        assert_eq!(clamp_scroll(99, 10, 4), 6);
        assert_eq!(clamp_scroll(2, 10, 4), 2);
    }

    #[test]
    fn does_not_scroll_when_everything_fits() {
        assert_eq!(clamp_scroll(5, 3, 10), 0);
    }

    #[test]
    fn detects_markdown_headings() {
        assert!(is_heading("# Title"));
        assert!(is_heading("  ### Nested"));
        assert!(!is_heading("not # a heading"));
        assert!(!is_heading(""));
    }
}
