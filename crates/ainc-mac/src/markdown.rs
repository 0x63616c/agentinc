//! Markdown for Evee's replies: paragraphs with inline styles, headings,
//! lists, quotes and fenced code. Parsed once per render from reply text.
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    Bold,
    Italic,
    Code,
    Link,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Inline {
    pub text: String,
    pub spans: Vec<(Range<usize>, Style)>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Block {
    Paragraph(Inline),
    Heading(u8, Inline),
    Code {
        language: Option<String>,
        text: String,
    },
    List {
        ordered: bool,
        items: Vec<Inline>,
    },
    Quote(Inline),
    Rule,
}

pub fn parse(source: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut inline = Inline::default();
    let mut open: Vec<(Style, usize)> = Vec::new();
    let mut heading: Option<u8> = None;
    let mut quote = 0usize;
    let mut lists: Vec<(bool, Vec<Inline>)> = Vec::new();
    let mut code: Option<(Option<String>, String)> = None;
    let mut list_depth_marker = String::new();
    let flush = |inline: &mut Inline,
                 blocks: &mut Vec<Block>,
                 heading: &mut Option<u8>,
                 quote: usize,
                 lists: &mut Vec<(bool, Vec<Inline>)>| {
        if inline.text.trim().is_empty() {
            *inline = Inline::default();
            return;
        }
        let taken = std::mem::take(inline);
        if let Some((_, items)) = lists.last_mut() {
            items.push(taken);
        } else if let Some(level) = heading.take() {
            blocks.push(Block::Heading(level, taken));
        } else if quote > 0 {
            blocks.push(Block::Quote(taken));
        } else {
            blocks.push(Block::Paragraph(taken));
        }
    };
    let parser = Parser::new_ext(
        source,
        Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES,
    );
    for event in parser {
        if let Some((_, text)) = code.as_mut() {
            match event {
                Event::Text(part) => text.push_str(&part),
                Event::End(TagEnd::CodeBlock) => {
                    let (language, mut text) = code.take().expect("open code block");
                    while text.ends_with('\n') {
                        text.pop();
                    }
                    blocks.push(Block::Code { language, text });
                }
                _ => {}
            }
            continue;
        }
        match event {
            Event::Start(Tag::CodeBlock(kind)) => {
                flush(&mut inline, &mut blocks, &mut heading, quote, &mut lists);
                let language = match kind {
                    CodeBlockKind::Fenced(lang) if !lang.trim().is_empty() => {
                        Some(lang.trim().to_owned())
                    }
                    _ => None,
                };
                code = Some((language, String::new()));
            }
            Event::Start(Tag::Heading { level, .. }) => {
                flush(&mut inline, &mut blocks, &mut heading, quote, &mut lists);
                heading = Some(level as u8);
            }
            Event::Start(Tag::BlockQuote(_)) => {
                flush(&mut inline, &mut blocks, &mut heading, quote, &mut lists);
                quote += 1;
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                flush(&mut inline, &mut blocks, &mut heading, quote, &mut lists);
                quote = quote.saturating_sub(1);
            }
            Event::Start(Tag::List(start)) => {
                flush(&mut inline, &mut blocks, &mut heading, quote, &mut lists);
                if !lists.is_empty() {
                    list_depth_marker.push_str("  ");
                }
                lists.push((start.is_some(), Vec::new()));
            }
            Event::End(TagEnd::List(_)) => {
                flush(&mut inline, &mut blocks, &mut heading, quote, &mut lists);
                if let Some((ordered, items)) = lists.pop() {
                    if let Some((_, parent)) = lists.last_mut() {
                        for item in items {
                            let mut nested = item;
                            nested.text = format!("{list_depth_marker}{}", nested.text);
                            let shift = list_depth_marker.len();
                            for (range, _) in &mut nested.spans {
                                range.start += shift;
                                range.end += shift;
                            }
                            parent.push(nested);
                        }
                        list_depth_marker.truncate(list_depth_marker.len().saturating_sub(2));
                    } else {
                        blocks.push(Block::List { ordered, items });
                    }
                }
            }
            Event::End(TagEnd::Item) => {
                flush(&mut inline, &mut blocks, &mut heading, quote, &mut lists);
            }
            Event::End(TagEnd::Paragraph) | Event::End(TagEnd::Heading(_)) => {
                if lists.is_empty() {
                    flush(&mut inline, &mut blocks, &mut heading, quote, &mut lists);
                } else if !inline.text.is_empty() && !inline.text.ends_with(' ') {
                    inline.text.push(' ');
                }
            }
            Event::Start(Tag::Strong) => open.push((Style::Bold, inline.text.len())),
            Event::End(TagEnd::Strong) => close(&mut inline, &mut open, Style::Bold),
            Event::Start(Tag::Emphasis) => open.push((Style::Italic, inline.text.len())),
            Event::End(TagEnd::Emphasis) => close(&mut inline, &mut open, Style::Italic),
            Event::Start(Tag::Link { .. }) => open.push((Style::Link, inline.text.len())),
            Event::End(TagEnd::Link) => close(&mut inline, &mut open, Style::Link),
            Event::Code(text) => {
                let start = inline.text.len();
                inline.text.push_str(&text);
                inline.spans.push((start..inline.text.len(), Style::Code));
            }
            Event::Text(text) => inline.text.push_str(&text),
            Event::SoftBreak => inline.text.push(' '),
            Event::HardBreak => inline.text.push('\n'),
            Event::Rule => {
                flush(&mut inline, &mut blocks, &mut heading, quote, &mut lists);
                blocks.push(Block::Rule);
            }
            Event::TaskListMarker(done) => inline.text.push_str(if done { "☑ " } else { "☐ " }),
            _ => {}
        }
    }
    flush(&mut inline, &mut blocks, &mut heading, quote, &mut lists);
    blocks
}
fn close(inline: &mut Inline, open: &mut Vec<(Style, usize)>, style: Style) {
    if let Some(index) = open.iter().rposition(|(s, _)| *s == style) {
        let (_, start) = open.remove(index);
        if start < inline.text.len() {
            inline.spans.push((start..inline.text.len(), style));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_reply_structure_with_inline_styles() {
        let blocks = parse(
            "# Plan\n\nStart with **bold** and `code` now.\n\n- one\n- two *soft*\n\n1. first\n\n```rust\nfn x() {}\n```\n\n> quoted\n\n---\n",
        );
        assert_eq!(blocks.len(), 7);
        assert!(matches!(&blocks[0], Block::Heading(1, inline) if inline.text == "Plan"));
        let Block::Paragraph(inline) = &blocks[1] else {
            panic!("paragraph")
        };
        assert_eq!(inline.text, "Start with bold and code now.");
        assert_eq!(inline.spans[0], (11..15, Style::Bold));
        assert_eq!(inline.spans[1], (20..24, Style::Code));
        let Block::List { ordered, items } = &blocks[2] else {
            panic!("list")
        };
        assert!(!ordered);
        assert_eq!(items.len(), 2);
        assert_eq!(items[1].text.trim(), "two soft");
        assert_eq!(items[1].spans[0].1, Style::Italic);
        assert!(matches!(&blocks[3], Block::List { ordered: true, .. }));
        assert_eq!(
            &blocks[4],
            &Block::Code {
                language: Some("rust".into()),
                text: "fn x() {}".into()
            }
        );
        assert!(matches!(&blocks[5], Block::Quote(inline) if inline.text == "quoted"));
        assert_eq!(blocks[6], Block::Rule);
    }
    #[test]
    fn plain_text_and_partial_markdown_stay_readable() {
        assert_eq!(parse("").len(), 0);
        let blocks = parse("Streaming **bo");
        let Block::Paragraph(inline) = &blocks[0] else {
            panic!("paragraph")
        };
        assert_eq!(inline.text, "Streaming **bo");
        let blocks = parse("```\nunterminated");
        assert_eq!(
            blocks,
            vec![Block::Code {
                language: None,
                text: "unterminated".into()
            }]
        );
    }
}

/// Reply text as elements. Inline styles become highlight runs; blocks stack
/// with even spacing so streaming drafts and finished replies look the same.
pub fn render(source: &str) -> gpui::Div {
    use crate::ui::*;
    use gpui::{FontWeight, div, prelude::*, px, rgb};
    let mut view = column().w_full().min_w_0().gap(px(8.));
    for block in parse(source) {
        view = view.child(match block {
            Block::Paragraph(inline) => inline_view(inline, LABEL_SIZE, FontWeight::NORMAL),
            Block::Heading(level, inline) => inline_view(
                inline,
                match level {
                    1 => BODY_SIZE + 4.,
                    2 => BODY_SIZE + 2.,
                    _ => BODY_SIZE,
                },
                FontWeight::SEMIBOLD,
            )
            .mt(px(4.)),
            Block::Code { language, text } => column()
                .w_full()
                .min_w_0()
                .rounded(px(8.))
                .bg(rgb(SURFACE_SEGMENT))
                .overflow_hidden()
                .when_some(language, |code, language| {
                    code.child(
                        div()
                            .px(px(12.))
                            .pt(px(8.))
                            .text_size(type_size(CAPTION_SIZE - 1.))
                            .text_color(rgb(MUTED))
                            .child(language),
                    )
                })
                .child(
                    div()
                        .px(px(12.))
                        .py(px(10.))
                        .font_family("SF Mono")
                        .text_size(type_size(CAPTION_SIZE))
                        .text_color(rgb(TEXT))
                        .child(text),
                ),
            Block::List { ordered, items } => {
                column()
                    .gap(px(4.))
                    .children(items.into_iter().enumerate().map(|(index, item)| {
                        row()
                            .gap(px(8.))
                            .items_start()
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .min_w(px(16.))
                                    .text_size(type_size(LABEL_SIZE))
                                    .text_color(rgb(MUTED))
                                    .child(if ordered {
                                        format!("{}.", index + 1)
                                    } else {
                                        "•".to_owned()
                                    }),
                            )
                            .child(
                                inline_view(item, LABEL_SIZE, FontWeight::NORMAL)
                                    .flex_1()
                                    .min_w_0(),
                            )
                    }))
            }
            Block::Quote(inline) => row()
                .gap(px(10.))
                .child(
                    div()
                        .w(px(2.))
                        .flex_shrink_0()
                        .rounded_full()
                        .bg(rgb(BORDER_OVERLAY)),
                )
                .child(
                    inline_view(inline, LABEL_SIZE, FontWeight::NORMAL)
                        .flex_1()
                        .min_w_0()
                        .text_color(rgb(MUTED)),
                ),
            Block::Rule => div().h(px(1.)).w_full().my(px(4.)).bg(rgb(BORDER)),
        });
    }
    view
}

fn inline_view(inline: Inline, points: f32, weight: gpui::FontWeight) -> gpui::Div {
    use crate::ui::*;
    use gpui::{
        FontStyle, FontWeight, HighlightStyle, StyledText, UnderlineStyle, div, prelude::*, px, rgb,
    };
    let highlights = segments(&inline)
        .into_iter()
        .map(|(range, styles)| {
            let mut highlight = HighlightStyle::default();
            for style in styles {
                match style {
                    Style::Bold => highlight.font_weight = Some(FontWeight::SEMIBOLD),
                    Style::Italic => highlight.font_style = Some(FontStyle::Italic),
                    Style::Code => {
                        highlight.background_color = Some(rgb(SURFACE_SEGMENT).into());
                        highlight.color = Some(rgb(TEXT_ACCENT).into());
                    }
                    Style::Link => {
                        highlight.color = Some(rgb(TEXT_ACCENT).into());
                        highlight.underline = Some(UnderlineStyle {
                            thickness: px(1.),
                            color: Some(rgb(TEXT_ACCENT).into()),
                            wavy: false,
                        });
                    }
                }
            }
            (range, highlight)
        })
        .collect::<Vec<_>>();
    div()
        .w_full()
        .min_w_0()
        .text_size(type_size(points))
        .font_weight(weight)
        .text_color(rgb(TEXT))
        .child(StyledText::new(inline.text).with_highlights(highlights))
}

/// Non-overlapping styled ranges in text order, since overlapping spans
/// (bold inside italic) must be split before they become text runs.
fn segments(inline: &Inline) -> Vec<(Range<usize>, Vec<Style>)> {
    let mut points: Vec<usize> = inline
        .spans
        .iter()
        .flat_map(|(range, _)| [range.start, range.end])
        .filter(|point| *point <= inline.text.len() && inline.text.is_char_boundary(*point))
        .collect();
    points.sort_unstable();
    points.dedup();
    let mut out = Vec::new();
    for pair in points.windows(2) {
        let (start, end) = (pair[0], pair[1]);
        let styles: Vec<Style> = inline
            .spans
            .iter()
            .filter(|(range, _)| range.start <= start && range.end >= end)
            .map(|(_, style)| *style)
            .collect();
        if !styles.is_empty() {
            out.push((start..end, styles));
        }
    }
    out
}

#[cfg(test)]
mod segment_tests {
    use super::*;
    #[test]
    fn overlapping_spans_split_into_ordered_segments() {
        let inline = Inline {
            text: "abcdef".into(),
            spans: vec![(0..4, Style::Italic), (2..6, Style::Bold)],
        };
        let segments = segments(&inline);
        assert_eq!(segments[0], (0..2, vec![Style::Italic]));
        assert_eq!(segments[1], (2..4, vec![Style::Italic, Style::Bold]));
        assert_eq!(segments[2], (4..6, vec![Style::Bold]));
    }
}
