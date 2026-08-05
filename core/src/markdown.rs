//! A line-oriented markdown renderer for the modal.
//!
//! It produces *semantics*, not escape codes: each line says what it is, and the plugin
//! maps that onto Zellij's styling primitives. That keeps the parsing testable natively
//! and leaves the terminal-specific part in the adapter, where it belongs.
//!
//! A terminal has one font size, so headings cannot literally be bigger. Hierarchy comes
//! from case, colour, weight and a rule underneath — which is what `LineKind` encodes.

/// What a line is. The plugin decides how each of these looks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineKind {
    Heading1,
    Heading2,
    Heading3,
    /// A horizontal rule. Its text is empty — the renderer draws it to the pane width.
    Rule,
    Checkbox {
        done: bool,
    },
    Bullet,
    Quote,
    Code,
    Plain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanKind {
    Strong,
    Code,
}

/// A styled run inside a line, in **character** offsets, end exclusive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub kind: SpanKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedLine {
    pub text: String,
    pub kind: LineKind,
    pub spans: Vec<Span>,
}

impl RenderedLine {
    fn new(text: impl Into<String>, kind: LineKind) -> Self {
        Self {
            text: text.into(),
            kind,
            spans: Vec::new(),
        }
    }
}

const CHECKED: &str = "☑";
const UNCHECKED: &str = "☐";
const BULLET: &str = "•";
const QUOTE_BAR: &str = "│";

/// Turns note content into displayable lines.
///
/// Headings lose their `#` markers and h1 is upper-cased; both h1 and h2 are followed by
/// a rule, which is what makes them read as headings without a larger font. Task list
/// markers become real checkbox glyphs.
pub fn render(content: &str) -> Vec<RenderedLine> {
    let mut out = Vec::new();
    let mut in_code_block = false;

    for raw in content.lines() {
        let trimmed = raw.trim_end();
        let stripped = trimmed.trim_start();

        if stripped.starts_with("```") || stripped.starts_with("~~~") {
            in_code_block = !in_code_block;
            out.push(RenderedLine::new("", LineKind::Rule));
            continue;
        }
        if in_code_block {
            out.push(RenderedLine::new(trimmed, LineKind::Code));
            continue;
        }
        if is_horizontal_rule(stripped) {
            out.push(RenderedLine::new("", LineKind::Rule));
            continue;
        }

        if let Some((kind, body)) = heading(stripped) {
            let text = if kind == LineKind::Heading1 {
                body.to_uppercase()
            } else {
                body.to_string()
            };
            let (text, spans) = inline(&text);
            out.push(RenderedLine {
                text,
                kind: kind.clone(),
                spans,
            });
            if kind != LineKind::Heading3 {
                out.push(RenderedLine::new("", LineKind::Rule));
            }
            continue;
        }

        if let Some((done, body)) = checkbox(stripped) {
            let glyph = if done { CHECKED } else { UNCHECKED };
            let (body, spans) = inline(body);
            let offset = glyph.chars().count() + 1;
            out.push(RenderedLine {
                text: format!("{glyph} {body}"),
                kind: LineKind::Checkbox { done },
                spans: shift(spans, offset),
            });
            continue;
        }

        if let Some(body) = bullet(stripped) {
            let (body, spans) = inline(body);
            out.push(RenderedLine {
                text: format!("{BULLET} {body}"),
                kind: LineKind::Bullet,
                spans: shift(spans, 2),
            });
            continue;
        }

        if let Some(body) = stripped.strip_prefix('>') {
            let (body, spans) = inline(body.trim_start());
            out.push(RenderedLine {
                text: format!("{QUOTE_BAR} {body}"),
                kind: LineKind::Quote,
                spans: shift(spans, 2),
            });
            continue;
        }

        let (text, spans) = inline(trimmed);
        out.push(RenderedLine {
            text,
            kind: LineKind::Plain,
            spans,
        });
    }
    out
}

/// Wraps rendered lines to `width`, carrying each line's kind onto its continuations and
/// clipping inline spans to the segment they land in.
pub fn wrap(lines: &[RenderedLine], width: usize) -> Vec<RenderedLine> {
    if width == 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for line in lines {
        for (start, segment) in segments(&line.text, width) {
            let end = start + segment.chars().count();
            let spans = line
                .spans
                .iter()
                .filter_map(|s| clip(*s, start, end))
                .collect();
            out.push(RenderedLine {
                text: segment,
                kind: line.kind.clone(),
                spans,
            });
        }
    }
    out
}

/// Splits a line into segments of at most `width` characters, each paired with its
/// starting character offset in the original. Offsets are preserved rather than
/// normalised, because inline spans are addressed by them.
fn segments(text: &str, width: usize) -> Vec<(usize, String)> {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= width {
        return vec![(0, text.to_string())];
    }
    let mut out = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        let hard_end = (start + width).min(chars.len());
        let mut end = hard_end;
        if hard_end < chars.len() {
            if let Some(space) = chars[start..hard_end].iter().rposition(|c| *c == ' ') {
                if space > 0 {
                    end = start + space;
                }
            }
        }
        out.push((start, chars[start..end].iter().collect()));
        start = if end < chars.len() && chars[end] == ' ' {
            end + 1
        } else {
            end
        };
    }
    out
}

fn clip(span: Span, start: usize, end: usize) -> Option<Span> {
    let s = span.start.max(start);
    let e = span.end.min(end);
    if s >= e {
        return None;
    }
    Some(Span {
        start: s - start,
        end: e - start,
        kind: span.kind,
    })
}

fn shift(spans: Vec<Span>, by: usize) -> Vec<Span> {
    spans
        .into_iter()
        .map(|s| Span {
            start: s.start + by,
            end: s.end + by,
            kind: s.kind,
        })
        .collect()
}

fn heading(line: &str) -> Option<(LineKind, &str)> {
    for (marker, kind) in [
        ("### ", LineKind::Heading3),
        ("## ", LineKind::Heading2),
        ("# ", LineKind::Heading1),
    ] {
        if let Some(body) = line.strip_prefix(marker) {
            return Some((kind, body.trim()));
        }
    }
    None
}

fn checkbox(line: &str) -> Option<(bool, &str)> {
    let body = bullet(line)?;
    if let Some(rest) = body.strip_prefix("[ ] ") {
        return Some((false, rest));
    }
    for marker in ["[x] ", "[X] "] {
        if let Some(rest) = body.strip_prefix(marker) {
            return Some((true, rest));
        }
    }
    None
}

fn bullet(line: &str) -> Option<&str> {
    for marker in ["- ", "* ", "+ "] {
        if let Some(body) = line.strip_prefix(marker) {
            return Some(body);
        }
    }
    None
}

fn is_horizontal_rule(line: &str) -> bool {
    let squeezed: String = line.chars().filter(|c| !c.is_whitespace()).collect();
    squeezed.len() >= 3
        && (squeezed.chars().all(|c| c == '-')
            || squeezed.chars().all(|c| c == '*')
            || squeezed.chars().all(|c| c == '_'))
}

/// Strips `**strong**` and `` `code` `` markers, reporting where the runs ended up.
/// An unmatched marker is left alone rather than swallowing the rest of the line.
fn inline(text: &str) -> (String, Vec<Span>) {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut spans = Vec::new();
    let mut len = 0;
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '`' {
            if let Some(close) = chars[i + 1..].iter().position(|c| *c == '`') {
                let close = i + 1 + close;
                let start = len;
                for c in &chars[i + 1..close] {
                    out.push(*c);
                    len += 1;
                }
                spans.push(Span {
                    start,
                    end: len,
                    kind: SpanKind::Code,
                });
                i = close + 1;
                continue;
            }
        }
        if chars[i] == '*' && chars.get(i + 1) == Some(&'*') {
            if let Some(close) = find_double_star(&chars, i + 2) {
                let start = len;
                for c in &chars[i + 2..close] {
                    out.push(*c);
                    len += 1;
                }
                spans.push(Span {
                    start,
                    end: len,
                    kind: SpanKind::Strong,
                });
                i = close + 2;
                continue;
            }
        }
        out.push(chars[i]);
        len += 1;
        i += 1;
    }
    (out, spans)
}

fn find_double_star(chars: &[char], from: usize) -> Option<usize> {
    let mut i = from;
    while i + 1 < chars.len() {
        if chars[i] == '*' && chars[i + 1] == '*' {
            return Some(i);
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(lines: &[RenderedLine]) -> Vec<LineKind> {
        lines.iter().map(|l| l.kind.clone()).collect()
    }

    fn texts(lines: &[RenderedLine]) -> Vec<String> {
        lines.iter().map(|l| l.text.clone()).collect()
    }

    #[test]
    fn upper_cases_h1_and_follows_it_with_a_rule() {
        let out = render("# Verifica");
        assert_eq!(texts(&out), vec!["VERIFICA", ""]);
        assert_eq!(kinds(&out), vec![LineKind::Heading1, LineKind::Rule]);
    }

    #[test]
    fn keeps_h2_case_and_still_rules_it() {
        let out = render("## Implementazione");
        assert_eq!(texts(&out), vec!["Implementazione", ""]);
        assert_eq!(kinds(&out), vec![LineKind::Heading2, LineKind::Rule]);
    }

    #[test]
    fn does_not_rule_h3() {
        assert_eq!(kinds(&render("### Dettaglio")), vec![LineKind::Heading3]);
    }

    #[test]
    fn a_hash_without_a_space_is_not_a_heading() {
        assert_eq!(kinds(&render("#hashtag")), vec![LineKind::Plain]);
    }

    #[test]
    fn turns_task_markers_into_checkboxes() {
        let out = render("- [ ] Composer 2.5\n- [x] Opus");
        assert_eq!(texts(&out), vec!["☐ Composer 2.5", "☑ Opus"]);
        assert_eq!(
            kinds(&out),
            vec![
                LineKind::Checkbox { done: false },
                LineKind::Checkbox { done: true }
            ]
        );
    }

    #[test]
    fn accepts_an_upper_case_x_as_done() {
        assert_eq!(
            kinds(&render("- [X] Grok")),
            vec![LineKind::Checkbox { done: true }]
        );
    }

    #[test]
    fn turns_plain_list_markers_into_bullets() {
        let out = render("- uno\n* due\n+ tre");
        assert_eq!(texts(&out), vec!["• uno", "• due", "• tre"]);
        assert_eq!(kinds(&out).len(), 3);
    }

    #[test]
    fn marks_quotes() {
        let out = render("> citazione");
        assert_eq!(texts(&out), vec!["│ citazione"]);
        assert_eq!(kinds(&out), vec![LineKind::Quote]);
    }

    #[test]
    fn recognises_horizontal_rules() {
        assert_eq!(kinds(&render("---")), vec![LineKind::Rule]);
        assert_eq!(kinds(&render("***")), vec![LineKind::Rule]);
        assert_eq!(kinds(&render("- - -")), vec![LineKind::Rule]);
        // Two dashes is not a rule, and a lone dash is a bullet marker.
        assert_eq!(kinds(&render("--")), vec![LineKind::Plain]);
    }

    #[test]
    fn keeps_code_blocks_verbatim() {
        let out = render("```rust\n# not a heading\n- [ ] not a checkbox\n```");
        assert_eq!(
            kinds(&out),
            vec![
                LineKind::Rule,
                LineKind::Code,
                LineKind::Code,
                LineKind::Rule
            ]
        );
        assert_eq!(texts(&out)[1], "# not a heading");
    }

    #[test]
    fn strips_strong_markers_and_reports_the_run() {
        let out = render("prima **forte** dopo");
        assert_eq!(out[0].text, "prima forte dopo");
        assert_eq!(
            out[0].spans,
            vec![Span {
                start: 6,
                end: 11,
                kind: SpanKind::Strong
            }]
        );
    }

    #[test]
    fn strips_code_markers_and_reports_the_run() {
        let out = render("usa `cargo test` ora");
        assert_eq!(out[0].text, "usa cargo test ora");
        assert_eq!(
            out[0].spans,
            vec![Span {
                start: 4,
                end: 14,
                kind: SpanKind::Code
            }]
        );
    }

    #[test]
    fn leaves_an_unmatched_marker_alone() {
        let out = render("2 * 3 = 6 e un `backtick");
        assert_eq!(out[0].text, "2 * 3 = 6 e un `backtick");
        assert!(out[0].spans.is_empty());
    }

    #[test]
    fn shifts_spans_past_a_checkbox_glyph() {
        let out = render("- [ ] usa `cargo`");
        assert_eq!(out[0].text, "☐ usa cargo");
        assert_eq!(
            out[0].spans,
            vec![Span {
                start: 6,
                end: 11,
                kind: SpanKind::Code
            }]
        );
    }

    #[test]
    fn wrapping_carries_the_kind_onto_continuations() {
        let lines = render("- [ ] uno due tre quattro");
        let wrapped = wrap(&lines, 10);
        assert!(wrapped.len() > 1);
        assert!(wrapped
            .iter()
            .all(|l| l.kind == LineKind::Checkbox { done: false }));
    }

    #[test]
    fn wrapping_clips_a_span_to_the_segment_holding_it() {
        // "una **frase** lunga da spezzare" -> strong run at chars 4..9
        let lines = render("una **frase** lunga da spezzare");
        let wrapped = wrap(&lines, 12);
        let carrying: Vec<_> = wrapped.iter().filter(|l| !l.spans.is_empty()).collect();
        assert_eq!(carrying.len(), 1, "the run belongs to exactly one segment");
        let span = carrying[0].spans[0];
        let slice: String = carrying[0]
            .text
            .chars()
            .skip(span.start)
            .take(span.end - span.start)
            .collect();
        assert_eq!(slice, "frase");
    }

    #[test]
    fn wrapping_a_zero_width_viewport_produces_nothing() {
        assert!(wrap(&render("qualcosa"), 0).is_empty());
    }

    #[test]
    fn hard_splits_a_word_longer_than_the_width() {
        let wrapped = wrap(&render("aaaaaaa"), 3);
        assert_eq!(texts(&wrapped), vec!["aaa", "aaa", "a"]);
    }
}
