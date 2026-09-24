// Adapted from GPUI 0.2.2 examples/input.rs (Apache-2.0). See THIRD_PARTY.md.
use std::ops::Range;

use crate::style::{FOCUS, TEXT, TEXT_SELECTION};
use gpui::prelude::*;
use gpui::*;
use unicode_segmentation::*;

actions!(
    text_input,
    [
        Backspace,
        Undo,
        Redo,
        Delete,
        Left,
        Right,
        SelectLeft,
        SelectRight,
        SelectAll,
        Home,
        End,
        ShowCharacterPalette,
        Paste,
        Cut,
        Copy,
        Quit,
    ]
);

#[derive(Clone, Copy, PartialEq)]
enum Boundary {
    Word,
    Line,
}
#[derive(Clone, PartialEq, Action)]
#[action(namespace = text_input, no_json)]
struct BoundaryEdit {
    boundary: Boundary,
    forward: bool,
    select: bool,
    delete: bool,
}
#[derive(Clone)]
struct Snapshot {
    content: SharedString,
    range: Range<usize>,
    reversed: bool,
}
fn word_boundary(text: &str, offset: usize, forward: bool) -> usize {
    if forward {
        text.unicode_word_indices()
            .map(|(start, word)| start + word.len())
            .find(|end| *end > offset)
            .unwrap_or(text.len())
    } else {
        text.unicode_word_indices()
            .rev()
            .map(|(start, _)| start)
            .find(|start| *start < offset)
            .unwrap_or(0)
    }
}
fn line_boundary(text: &str, offset: usize, forward: bool) -> usize {
    if forward {
        text[offset..].find('\n').map_or(text.len(), |i| offset + i)
    } else {
        text[..offset].rfind('\n').map_or(0, |i| i + 1)
    }
}
pub struct Submit;
impl EventEmitter<Submit> for TextInput {}

pub struct TextInput {
    secret: bool,
    multiline: bool,
    multiline_layout: Vec<(usize, ShapedLine, Bounds<Pixels>)>,
    submit: bool,
    scroll_x: Pixels,
    focus_handle: FocusHandle,
    pub content: SharedString,
    placeholder: SharedString,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    last_layout: Option<ShapedLine>,
    last_bounds: Option<Bounds<Pixels>>,
    is_selecting: bool,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
}

impl TextInput {
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            content: self.content.clone(),
            range: self.selected_range.clone(),
            reversed: self.selection_reversed,
        }
    }
    fn restore(&mut self, snapshot: Snapshot, cx: &mut Context<Self>) {
        self.content = snapshot.content;
        self.selected_range = snapshot.range;
        self.selection_reversed = snapshot.reversed;
        self.marked_range = None;
        cx.notify();
    }
    fn undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(snapshot) = self.undo.pop() {
            self.redo.push(self.snapshot());
            self.restore(snapshot, cx);
        }
    }
    fn redo(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(snapshot) = self.redo.pop() {
            self.undo.push(self.snapshot());
            self.restore(snapshot, cx);
        }
    }
    fn boundary_edit(
        &mut self,
        action: &BoundaryEdit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let offset = match action.boundary {
            Boundary::Word => word_boundary(&self.content, self.cursor_offset(), action.forward),
            Boundary::Line => line_boundary(&self.content, self.cursor_offset(), action.forward),
        };
        if action.delete {
            if self.selected_range.is_empty() {
                self.select_to(offset, cx);
            }
            self.replace_text_in_range(None, "", window, cx);
        } else if action.select {
            self.select_to(offset, cx);
        } else {
            self.move_to(offset, cx);
        }
    }
    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.start, cx)
        }
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.selected_range.end), cx);
        } else {
            self.move_to(self.selected_range.end, cx)
        }
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
        self.select_to(self.content.len(), cx)
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(
            line_boundary(&self.content, self.cursor_offset(), false),
            cx,
        );
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(line_boundary(&self.content, self.cursor_offset(), true), cx);
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.previous_boundary(self.cursor_offset()), cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.next_boundary(self.cursor_offset()), cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.is_selecting = true;

        if event.click_count >= 3 {
            self.move_to(0, cx);
            self.select_to(self.content.len(), cx);
        } else if event.click_count == 2 {
            let offset = self.index_for_mouse_position(event.position);
            let range = self
                .content
                .unicode_word_indices()
                .find(|(start, word)| *start <= offset && offset <= *start + word.len())
                .map(|(start, word)| start..start + word.len())
                .unwrap_or(offset..offset);
            self.move_to(range.start, cx);
            self.select_to(range.end, cx);
        } else if event.modifiers.shift {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        } else {
            self.move_to(self.index_for_mouse_position(event.position), cx)
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _window: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        }
    }

    fn show_character_palette(
        &mut self,
        _: &ShowCharacterPalette,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        window.show_character_palette();
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.replace_text_in_range(None, &text, window, cx);
        }
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.secret && !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
        }
    }
    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            if !self.secret {
                cx.write_to_clipboard(ClipboardItem::new_string(
                    self.content[self.selected_range.clone()].to_string(),
                ));
            }
            self.replace_text_in_range(None, "", window, cx)
        }
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        self.selection_reversed = false;
        cx.notify()
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        if self.content.is_empty() {
            return 0;
        }

        if self.multiline {
            if let Some((offset, line, bounds)) = self
                .multiline_layout
                .iter()
                .find(|(_, _, b)| position.y < b.bottom())
                .or(self.multiline_layout.last())
            {
                return offset + line.closest_index_for_x(position.x - bounds.left());
            }
            return 0;
        }
        let (Some(bounds), Some(line)) = (self.last_bounds.as_ref(), self.last_layout.as_ref())
        else {
            return 0;
        };
        if position.y < bounds.top() {
            return 0;
        }
        if position.y > bounds.bottom() {
            return self.content.len();
        }
        let index = line.closest_index_for_x(position.x - bounds.left());
        self.content
            .floor_char_boundary(index.min(self.content.len()))
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        if self.selection_reversed {
            self.selected_range.start = offset
        } else {
            self.selected_range.end = offset
        };
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        cx.notify()
    }

    fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8_offset = 0;
        let mut utf16_count = 0;

        for ch in self.content.chars() {
            if utf16_count >= offset {
                break;
            }
            utf16_count += ch.len_utf16();
            utf8_offset += ch.len_utf8();
        }

        utf8_offset
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf16_offset = 0;
        let mut utf8_count = 0;

        for ch in self.content.chars() {
            if utf8_count >= offset {
                break;
            }
            utf8_count += ch.len_utf8();
            utf16_offset += ch.len_utf16();
        }

        utf16_offset
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range_utf16.start)..self.offset_from_utf16(range_utf16.end)
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .rev()
            .find_map(|(idx, _)| (idx < offset).then_some(idx))
            .unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .find_map(|(idx, _)| (idx > offset).then_some(idx))
            .unwrap_or(self.content.len())
    }

    pub fn field(placeholder: &str, secret: bool, cx: &mut Context<Self>) -> Self {
        let mut input = Self::new(cx);
        input.placeholder = placeholder.to_owned().into();
        input.secret = secret;
        input.submit = true;
        input
    }
    pub fn composer(cx: &mut Context<Self>) -> Self {
        let mut input = Self::field("Ask Evee…", false, cx);
        input.multiline = true;
        input
    }
    pub fn set_text(&mut self, text: &str, cx: &mut Context<Self>) {
        self.reset();
        self.content = text.to_owned().into();
        self.selected_range = text.len()..text.len();
        cx.notify();
    }
    pub fn reset(&mut self) {
        self.scroll_x = px(0.);
        self.content = "".into();
        self.selected_range = 0..0;
        self.selection_reversed = false;
        self.marked_range = None;
        self.last_layout = None;
        self.last_bounds = None;
        self.is_selecting = false;
        self.undo.clear();
        self.redo.clear();
    }
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        if self.secret {
            return None;
        }
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.content[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.marked_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if (self.secret && !new_text.is_ascii()) || self.content.len() + new_text.len() > 65536 {
            return;
        }
        let single_line = if self.multiline {
            new_text.replace("\r\n", "\n").replace('\r', "\n")
        } else {
            new_text.replace(['\n', '\r'], " ")
        };
        let new_text = single_line.as_str();
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

        if self.undo.len() == 100 {
            self.undo.remove(0);
        }
        self.undo.push(self.snapshot());
        self.redo.clear();
        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        self.selected_range = range.start + new_text.len()..range.start + new_text.len();
        self.marked_range.take();
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if (self.secret && !new_text.is_ascii()) || self.content.len() + new_text.len() > 65536 {
            return;
        }
        let single_line = if self.multiline {
            new_text.replace("\r\n", "\n").replace('\r', "\n")
        } else {
            new_text.replace(['\n', '\r'], " ")
        };
        let new_text = single_line.as_str();
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        if !new_text.is_empty() {
            self.marked_range = Some(range.start..range.start + new_text.len());
        } else {
            self.marked_range = None;
        }
        self.selected_range = new_selected_range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .map(|new_range| new_range.start + range.start..new_range.end + range.start)
            .unwrap_or_else(|| range.start + new_text.len()..range.start + new_text.len());

        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let range = self.range_from_utf16(&range_utf16);
        if self.multiline {
            let (offset, line, bounds) = self
                .multiline_layout
                .iter()
                .rev()
                .find(|(offset, _, _)| *offset <= range.start)?;
            return Some(Bounds::new(
                point(
                    bounds.left() + line.x_for_index(range.start - offset),
                    bounds.top(),
                ),
                size(px(2.), bounds.size.height),
            ));
        }
        let last_layout = self.last_layout.as_ref()?;
        Some(Bounds::from_corners(
            point(
                bounds.left() + last_layout.x_for_index(range.start),
                bounds.top(),
            ),
            point(
                bounds.left() + last_layout.x_for_index(range.end),
                bounds.bottom(),
            ),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: gpui::Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        if self.multiline {
            return Some(self.offset_to_utf16(self.index_for_mouse_position(point)));
        }
        let line_point = self.last_bounds?.localize(&point)?;
        let last_layout = self.last_layout.as_ref()?;

        let utf8_index = last_layout.index_for_x(line_point.x)?;
        Some(self.offset_to_utf16(utf8_index))
    }
}

struct TextElement {
    input: Entity<TextInput>,
}

struct PrepaintState {
    multiline: Vec<(usize, ShapedLine, Bounds<Pixels>)>,
    selections: Vec<PaintQuad>,
    line: Option<ShapedLine>,
    scroll_x: Pixels,
    cursor: Option<PaintQuad>,
    selection: Option<PaintQuad>,
}

impl IntoElement for TextElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        let input = self.input.read(cx);
        let lines = if input.multiline {
            input.content.split('\n').count().clamp(2, 5)
        } else {
            1
        };
        style.size.height = (window.line_height() * lines as f32).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let input = self.input.read(cx);
        let content = input.content.clone();
        let selected_range = input.selected_range.clone();
        let cursor = input.cursor_offset();
        let style = window.text_style();
        if input.multiline {
            let text = if content.is_empty() {
                input.placeholder.clone()
            } else {
                content.clone()
            };
            let height = window.line_height();
            let cursor_row = content[..cursor].bytes().filter(|c| *c == b'\n').count();
            let first_row = cursor_row.saturating_sub(4);
            let mut offset = 0;
            let mut lines = Vec::new();
            let mut selections = Vec::new();
            let mut caret = None;
            let cursor_start = content[..cursor].rfind('\n').map_or(0, |i| i + 1);
            let cursor_text: SharedString = content[cursor_start..cursor].to_owned().into();
            let font_size = style.font_size.to_pixels(window.rem_size());
            let color = if content.is_empty() {
                rgb(0x888888).into()
            } else {
                style.color
            };
            let run = TextRun {
                len: cursor_text.len(),
                font: style.font(),
                color,
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let cursor_width = window
                .text_system()
                .shape_line(cursor_text, font_size, &[run], None)
                .width;
            let scroll_x = (cursor_width - bounds.size.width + px(3.)).max(px(0.));
            for (row, text) in text.split('\n').enumerate() {
                let run = TextRun {
                    len: text.len(),
                    font: style.font(),
                    color,
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                };
                let line = window.text_system().shape_line(
                    text.to_owned().into(),
                    font_size,
                    &[run],
                    None,
                );
                let origin = point(
                    bounds.left() - scroll_x,
                    bounds.top() + height * (row as f32 - first_row as f32),
                );
                let line_bounds = Bounds::new(origin, size(bounds.size.width, height));
                if row == cursor_row {
                    caret = Some(fill(
                        Bounds::new(
                            point(origin.x + line.x_for_index(cursor - offset), origin.y),
                            size(px(2.), height),
                        ),
                        rgb(FOCUS),
                    ));
                }
                let start = selected_range.start.max(offset);
                let end = selected_range.end.min(offset + text.len());
                if start < end {
                    selections.push(fill(
                        Bounds::new(
                            point(origin.x + line.x_for_index(start - offset), origin.y),
                            size(
                                line.x_for_index(end - offset) - line.x_for_index(start - offset),
                                height,
                            ),
                        ),
                        rgba(TEXT_SELECTION),
                    ));
                }
                lines.push((offset, line, line_bounds));
                offset += text.len() + 1;
            }
            return PrepaintState {
                multiline: lines,
                selections,
                line: None,
                scroll_x,
                cursor: caret,
                selection: None,
            };
        }

        let (display_text, text_color) = if content.is_empty() {
            (input.placeholder.clone(), rgb(0x888888).into())
        } else if input.secret {
            ("*".repeat(content.len()).into(), style.color)
        } else {
            (content, style.color)
        };

        let run = TextRun {
            len: display_text.len(),
            font: style.font(),
            color: text_color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let runs = if let Some(marked_range) = input.marked_range.as_ref() {
            vec![
                TextRun {
                    len: marked_range.start,
                    ..run.clone()
                },
                TextRun {
                    len: marked_range.end - marked_range.start,
                    underline: Some(UnderlineStyle {
                        color: Some(run.color),
                        thickness: px(1.0),
                        wavy: false,
                    }),
                    ..run.clone()
                },
                TextRun {
                    len: display_text.len() - marked_range.end,
                    ..run
                },
            ]
            .into_iter()
            .filter(|run| run.len > 0)
            .collect()
        } else {
            vec![run]
        };

        let font_size = style.font_size.to_pixels(window.rem_size());
        let line = window
            .text_system()
            .shape_line(display_text, font_size, &runs, None);

        let cursor_pos = line.x_for_index(cursor);
        let available = (bounds.size.width - px(3.)).max(px(0.));
        let scroll_x = input
            .scroll_x
            .min(cursor_pos)
            .max(cursor_pos - available)
            .max(px(0.));
        let bounds = Bounds::new(bounds.origin - point(scroll_x, px(0.)), bounds.size);
        let (selection, cursor) = if selected_range.is_empty() {
            (
                None,
                Some(fill(
                    Bounds::new(
                        point(bounds.left() + cursor_pos, bounds.top()),
                        size(px(2.), bounds.bottom() - bounds.top()),
                    ),
                    rgb(FOCUS),
                )),
            )
        } else {
            (
                Some(fill(
                    Bounds::from_corners(
                        point(
                            bounds.left() + line.x_for_index(selected_range.start),
                            bounds.top(),
                        ),
                        point(
                            bounds.left() + line.x_for_index(selected_range.end),
                            bounds.bottom(),
                        ),
                    ),
                    rgba(TEXT_SELECTION),
                )),
                None,
            )
        };
        PrepaintState {
            multiline: vec![],
            selections: vec![],
            scroll_x,
            line: Some(line),
            cursor,
            selection,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        if self.input.read(cx).multiline {
            for selection in prepaint.selections.drain(..) {
                window.paint_quad(selection);
            }
            for (_, line, line_bounds) in &prepaint.multiline {
                let _ = line.paint(line_bounds.origin, window.line_height(), window, cx);
            }
            if focus_handle.is_focused(window)
                && let Some(cursor) = prepaint.cursor.take()
            {
                window.paint_quad(cursor);
            }
            self.input.update(cx, |input, _| {
                input.multiline_layout = std::mem::take(&mut prepaint.multiline);
                input.last_bounds = Some(bounds);
            });
            return;
        }
        if let Some(selection) = prepaint.selection.take() {
            window.paint_quad(selection)
        }
        let line = prepaint.line.take().unwrap();
        let text_bounds = Bounds::new(
            bounds.origin - point(prepaint.scroll_x, px(0.)),
            bounds.size,
        );
        line.paint(text_bounds.origin, window.line_height(), window, cx)
            .unwrap();

        if focus_handle.is_focused(window)
            && let Some(cursor) = prepaint.cursor.take()
        {
            window.paint_quad(cursor);
        }

        self.input.update(cx, |input, _cx| {
            input.last_layout = Some(line);
            input.last_bounds = Some(text_bounds);
            input.scroll_x = prepaint.scroll_x;
        });
    }
}

impl Render for TextInput {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .key_context("TextInput")
            .track_focus(&self.focus_handle(cx))
            .overflow_hidden()
            .cursor(CursorStyle::IBeam)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if this.multiline
                    && this.marked_range.is_none()
                    && matches!(event.keystroke.key.as_str(), "up" | "down")
                {
                    let offset = this.cursor_offset();
                    if let Some((start, line, bounds)) = this
                        .multiline_layout
                        .iter()
                        .rev()
                        .find(|(start, _, _)| *start <= offset)
                    {
                        let position = point(
                            bounds.left() + line.x_for_index(offset - *start),
                            bounds.top()
                                + bounds.size.height
                                    * if event.keystroke.key == "up" {
                                        -0.5
                                    } else {
                                        1.5
                                    },
                        );
                        let target = this.index_for_mouse_position(position);
                        if event.keystroke.modifiers.shift {
                            this.select_to(target, cx)
                        } else {
                            this.move_to(target, cx)
                        }
                    }
                    cx.stop_propagation();
                    return;
                }
                if this.submit && event.keystroke.key == "enter" && this.marked_range.is_none() {
                    cx.stop_propagation();
                    if this.multiline && event.keystroke.modifiers.shift {
                        this.replace_text_in_range(None, "\n", window, cx);
                    } else {
                        cx.emit(Submit);
                    }
                }
            }))
            .on_action(cx.listener(Self::boundary_edit))
            .on_action(cx.listener(Self::undo))
            .on_action(cx.listener(Self::redo))
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::show_character_palette))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .w_full()
            .overflow_hidden()
            .line_height(px(20.))
            .text_size(px(13.))
            .text_color(rgb(TEXT))
            .child(TextElement { input: cx.entity() })
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl TextInput {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            secret: false,
            multiline: false,
            multiline_layout: vec![],
            submit: false,
            scroll_x: px(0.),
            focus_handle: cx.focus_handle(),
            content: "".into(),
            placeholder: "Go to…".into(),
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            last_layout: None,
            last_bounds: None,
            is_selecting: false,
            undo: vec![],
            redo: vec![],
        }
    }
}
pub fn bind_keys(cx: &mut App) {
    for (key, boundary, forward, select, delete) in [
        ("alt-left", Boundary::Word, false, false, false),
        ("alt-right", Boundary::Word, true, false, false),
        ("alt-shift-left", Boundary::Word, false, true, false),
        ("alt-shift-right", Boundary::Word, true, true, false),
        ("cmd-shift-left", Boundary::Line, false, true, false),
        ("cmd-shift-right", Boundary::Line, true, true, false),
        ("shift-home", Boundary::Line, false, true, false),
        ("shift-end", Boundary::Line, true, true, false),
        ("alt-backspace", Boundary::Word, false, false, true),
        ("alt-delete", Boundary::Word, true, false, true),
        ("cmd-backspace", Boundary::Line, false, false, true),
        ("cmd-delete", Boundary::Line, true, false, true),
        ("ctrl-k", Boundary::Line, true, false, true),
        ("ctrl-w", Boundary::Word, false, false, true),
    ] {
        cx.bind_keys([KeyBinding::new(
            key,
            BoundaryEdit {
                boundary,
                forward,
                select,
                delete,
            },
            Some("TextInput"),
        )]);
    }
    cx.bind_keys([
        KeyBinding::new("cmd-z", Undo, Some("TextInput")),
        KeyBinding::new("cmd-shift-z", Redo, Some("TextInput")),
        KeyBinding::new("ctrl-a", Home, Some("TextInput")),
        KeyBinding::new("ctrl-e", End, Some("TextInput")),
        KeyBinding::new("ctrl-b", Left, Some("TextInput")),
        KeyBinding::new("ctrl-f", Right, Some("TextInput")),
        KeyBinding::new("ctrl-h", Backspace, Some("TextInput")),
        KeyBinding::new("ctrl-d", Delete, Some("TextInput")),
        KeyBinding::new("backspace", Backspace, Some("TextInput")),
        KeyBinding::new("delete", Delete, Some("TextInput")),
        KeyBinding::new("left", Left, Some("TextInput")),
        KeyBinding::new("right", Right, Some("TextInput")),
        KeyBinding::new("shift-left", SelectLeft, Some("TextInput")),
        KeyBinding::new("shift-right", SelectRight, Some("TextInput")),
        KeyBinding::new("cmd-a", SelectAll, Some("TextInput")),
        KeyBinding::new("cmd-v", Paste, Some("TextInput")),
        KeyBinding::new("cmd-c", Copy, Some("TextInput")),
        KeyBinding::new("cmd-x", Cut, Some("TextInput")),
        KeyBinding::new("home", Home, Some("TextInput")),
        KeyBinding::new("end", End, Some("TextInput")),
        KeyBinding::new("cmd-left", Home, Some("TextInput")),
        KeyBinding::new("cmd-right", End, Some("TextInput")),
        KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, Some("TextInput")),
    ]);
}

#[cfg(test)]
mod tests {
    use super::{line_boundary, word_boundary};
    #[test]
    fn multiline_boundaries_preserve_other_lines() {
        let text = "one\ncafé\nthree";
        assert_eq!(line_boundary(text, 7, false), 4);
        assert_eq!(line_boundary(text, 7, true), 9);
        assert_eq!(line_boundary(text, 4, false), 4);
        assert_eq!(line_boundary(text, text.len(), true), text.len());
    }
    #[test]
    fn word_navigation_respects_unicode_and_punctuation() {
        let text = "hello, café world";
        assert_eq!(word_boundary(text, text.len(), false), 13);
        assert_eq!(word_boundary(text, 13, false), 7);
        assert_eq!(word_boundary(text, 0, true), 5);
        assert_eq!(word_boundary(text, 5, true), 12);
        assert_eq!(word_boundary("👋 café", 0, true), "👋 café".len());
        assert_eq!(word_boundary("", 0, false), 0);
    }
}
