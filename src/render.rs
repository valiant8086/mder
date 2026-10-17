//! Markdown to HTML, with every block tagged by the source line it starts on.
//!
//! Blocks get `data-line="N"` (1-based). The page uses it two ways: Enter on a
//! block reports its line to the editor, and the editor asks the page to
//! focus the block for the caret's line.
//!
//! The leading text of a list item is wrapped in `<span class="li-text">`.
//! The page focuses that span instead of the `<li>`: a focusable list item
//! makes NVDA switch to focus mode.

use pulldown_cmark::{CowStr, Event, Options, Parser, Tag, TagEnd, html};

pub fn to_html(source: &str) -> String {
    let lines = LineIndex::new(source);
    let parser = Parser::new_ext(source, options()).into_offset_iter();

    // One entry per open list item: is its leading-text span open?
    let mut items: Vec<ItemSpan> = Vec::new();
    let mut events: Vec<Event> = Vec::new();

    for (event, range) in parser {
        let line = lines.line_of(range.start);
        let block_start = starts_block(&event);

        if let Some(item) = items.last_mut() {
            match *item {
                ItemSpan::Pending if !block_start && !matches!(event, Event::End(TagEnd::Item)) => {
                    events.push(raw("<span class=\"li-text\">"));
                    *item = ItemSpan::Open;
                }
                ItemSpan::Pending if block_start => *item = ItemSpan::Closed,
                ItemSpan::Open if block_start => {
                    events.push(raw("</span>"));
                    *item = ItemSpan::Closed;
                }
                _ => {}
            }
        }

        match event {
            Event::Start(Tag::Paragraph) => events.push(raw(format!("<p data-line=\"{line}\">"))),
            Event::End(TagEnd::Paragraph) => events.push(raw("</p>\n")),
            Event::Start(Tag::Heading { level, .. }) => {
                events.push(raw(format!("<{level} data-line=\"{line}\">")));
            }
            Event::End(TagEnd::Heading(level)) => events.push(raw(format!("</{level}>\n"))),
            Event::Start(Tag::BlockQuote(_)) => {
                events.push(raw(format!("<blockquote data-line=\"{line}\">\n")));
            }
            Event::End(TagEnd::BlockQuote(_)) => events.push(raw("</blockquote>\n")),
            Event::Start(Tag::CodeBlock(kind)) => {
                let lang = match &kind {
                    pulldown_cmark::CodeBlockKind::Fenced(info) => info.split_whitespace().next().unwrap_or(""),
                    pulldown_cmark::CodeBlockKind::Indented => "",
                };
                let lang: String = lang.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
                let class = if lang.is_empty() { String::new() } else { format!(" class=\"language-{lang}\"") };
                events.push(raw(format!("<pre data-line=\"{line}\"><code{class}>")));
            }
            Event::End(TagEnd::CodeBlock) => events.push(raw("</code></pre>\n")),
            Event::Start(Tag::Item) => {
                events.push(raw(format!("<li data-line=\"{line}\">")));
                items.push(ItemSpan::Pending);
            }
            Event::End(TagEnd::Item) => {
                if let Some(ItemSpan::Open) = items.pop() {
                    events.push(raw("</span>"));
                }
                events.push(raw("</li>\n"));
            }
            // Tables keep pulldown-cmark's own tags (it tracks column
            // alignment), so the line goes on a wrapper.
            Event::Start(Tag::Table(_)) => {
                events.push(raw(format!("<div data-line=\"{line}\">\n")));
                events.push(event);
            }
            Event::End(TagEnd::Table) => {
                events.push(event);
                events.push(raw("</div>\n"));
            }
            Event::Rule => events.push(raw(format!("<hr data-line=\"{line}\">\n"))),
            other => events.push(other),
        }
    }

    let mut out = String::with_capacity(source.len() * 2);
    html::push_html(&mut out, events.into_iter());
    out
}

#[derive(Clone, Copy)]
enum ItemSpan {
    /// Nothing seen yet in this item.
    Pending,
    /// Inline content came first and the span is open.
    Open,
    /// A block came first, or the span was already closed.
    Closed,
}

fn options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES
}

fn raw<'a>(s: impl Into<CowStr<'a>>) -> Event<'a> {
    Event::Html(s.into())
}

fn starts_block(event: &Event) -> bool {
    matches!(
        event,
        Event::Start(
            Tag::Paragraph
                | Tag::Heading { .. }
                | Tag::BlockQuote(_)
                | Tag::CodeBlock(_)
                | Tag::List(_)
                | Tag::Item
                | Tag::Table(_)
                | Tag::HtmlBlock
                | Tag::FootnoteDefinition(_)
        ) | Event::Rule
    )
}

/// The line of the heading whose GitHub-style anchor is `anchor`.
pub fn heading_line(source: &str, anchor: &str) -> Option<usize> {
    let anchor = anchor.trim_start_matches('#').to_lowercase();
    let lines = LineIndex::new(source);
    let mut seen: Vec<String> = Vec::new();
    let mut current: Option<(usize, String)> = None;

    for (event, range) in Parser::new_ext(source, options()).into_offset_iter() {
        match event {
            Event::Start(Tag::Heading { .. }) => current = Some((lines.line_of(range.start), String::new())),
            Event::Text(t) | Event::Code(t) => {
                if let Some((_, text)) = current.as_mut() {
                    text.push_str(&t);
                }
            }
            Event::End(TagEnd::Heading(_)) => {
                let Some((line, text)) = current.take() else { continue };
                let base = slug(&text);
                let n = seen.iter().filter(|s| **s == base).count();
                let unique = if n == 0 { base.clone() } else { format!("{base}-{n}") };
                seen.push(base);
                if unique == anchor {
                    return Some(line);
                }
            }
            _ => {}
        }
    }
    None
}

/// GitHub's heading anchor: lowercase, punctuation dropped, spaces to hyphens.
fn slug(text: &str) -> String {
    text.trim()
        .to_lowercase()
        .chars()
        .filter_map(|c| match c {
            ' ' => Some('-'),
            c if c.is_alphanumeric() || c == '-' || c == '_' => Some(c),
            _ => None,
        })
        .collect()
}

/// Maps byte offsets in the source to 1-based line numbers.
pub struct LineIndex {
    starts: Vec<usize>,
}

impl LineIndex {
    pub fn new(text: &str) -> Self {
        let mut starts = vec![0];
        starts.extend(text.match_indices('\n').map(|(i, _)| i + 1));
        Self { starts }
    }

    pub fn line_of(&self, offset: usize) -> usize {
        self.starts.partition_point(|&s| s <= offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_carry_their_line() {
        let html = to_html("# Title\n\nSome text.\n\n- one\n- two\n");
        assert!(html.contains("<h1 data-line=\"1\">Title</h1>"), "{html}");
        assert!(html.contains("<p data-line=\"3\">Some text.</p>"), "{html}");
        assert!(html.contains("<li data-line=\"6\"><span class=\"li-text\">two</span></li>"), "{html}");
    }

    #[test]
    fn nested_list_closes_the_span_first() {
        let html = to_html("- outer\n  - inner\n");
        assert!(html.contains("<li data-line=\"1\"><span class=\"li-text\">outer</span>\n<ul>"), "{html}");
        assert!(html.contains("<li data-line=\"2\"><span class=\"li-text\">inner</span></li>"), "{html}");
    }

    #[test]
    fn loose_items_have_no_span() {
        let html = to_html("- one\n\n- two\n");
        assert!(html.contains("<li data-line=\"1\">\n<p data-line=\"1\">one</p>") || html.contains("<li data-line=\"1\"><p data-line=\"1\">one</p>"), "{html}");
        assert!(!html.contains("li-text"), "{html}");
    }

    #[test]
    fn anchors_find_headings() {
        let src = "# Intro\n\ntext\n\n## Setup & Use\n\n## Setup & Use\n\n### `code` Here\n";
        assert_eq!(heading_line(src, "intro"), Some(1));
        assert_eq!(heading_line(src, "#setup--use"), Some(5));
        assert_eq!(heading_line(src, "setup--use-1"), Some(7));
        assert_eq!(heading_line(src, "code-here"), Some(9));
        assert_eq!(heading_line(src, "missing"), None);
    }

    #[test]
    fn table_and_code_get_lines() {
        let html = to_html("| a | b |\n|---|---|\n| 1 | 2 |\n\n```rust\nfn x() {}\n```\n");
        assert!(html.contains("<div data-line=\"1\">"), "{html}");
        assert!(html.contains("<pre data-line=\"5\"><code class=\"language-rust\">"), "{html}");
    }
}
