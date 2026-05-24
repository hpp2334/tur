use boa_gc::{Finalize, Trace};

use crate::elements::text::span_data::SpanData;

#[derive(Trace, Finalize, boa_engine::JsData)]
#[boa_gc(unsafe_empty_trace)]
pub struct TextEditingController {
    spans: Vec<SpanData>,
    cursor_position: usize,
    selection_anchor: usize,
    selection_end: usize,
    composing_text: Option<String>,
    composing_start: usize,
}

impl TextEditingController {
    pub fn new() -> Self {
        Self {
            spans: Vec::new(),
            cursor_position: 0,
            selection_anchor: 0,
            selection_end: 0,
            composing_text: None,
            composing_start: 0,
        }
    }

    pub fn text(&self) -> String {
        self.spans.iter().map(|s| s.text.as_str()).collect()
    }

    pub fn spans(&self) -> &[SpanData] {
        &self.spans
    }

    pub fn set_spans(&mut self, spans: Vec<SpanData>) {
        self.spans = spans;
        self.cursor_position = self.full_len();
        self.selection_anchor = self.cursor_position;
        self.selection_end = self.cursor_position;
        self.composing_text = None;
    }

    pub fn clear(&mut self) {
        self.spans.clear();
        self.cursor_position = 0;
        self.selection_anchor = 0;
        self.selection_end = 0;
        self.composing_text = None;
        self.composing_start = 0;
    }

    pub fn cursor_position(&self) -> usize {
        self.cursor_position
    }

    pub fn set_cursor_position(&mut self, pos: usize) {
        self.cursor_position = pos;
    }

    pub fn selection_anchor(&self) -> usize {
        self.selection_anchor
    }

    pub fn selection_end(&self) -> usize {
        self.selection_end
    }

    pub fn has_selection(&self) -> bool {
        self.selection_anchor != self.selection_end
    }

    pub fn selection_range(&self) -> (usize, usize) {
        let (a, b) = (self.selection_anchor, self.selection_end);
        if a <= b { (a, b) } else { (b, a) }
    }

    pub fn clear_selection(&mut self) {
        self.selection_anchor = self.cursor_position;
        self.selection_end = self.cursor_position;
    }

    pub fn delete_selection(&mut self) {
        if !self.has_selection() { return; }
        let (start, end) = self.selection_range();
        self.delete_range(start, end);
        self.cursor_position = start;
        self.clear_selection();
    }

    pub fn is_composing(&self) -> bool {
        self.composing_text.is_some()
    }

    pub fn full_len(&self) -> usize {
        self.spans.iter().map(|s| s.text.len()).sum()
    }

    pub fn insert_char_at(&mut self, pos: usize, ch: char) {
        self.insert_at(pos, &ch.to_string());
    }

    pub fn insert_str_at(&mut self, pos: usize, text: &str) {
        self.insert_at(pos, text);
    }

    pub fn composition_display_text(&self) -> String {
        let base = self.text();
        if let Some(ref comp) = self.composing_text {
            let start = self.composing_start.min(base.len());
            format!("{}{}{}", &base[..start], comp, &base[start..])
        } else {
            base
        }
    }

    pub fn start_composition(&mut self) {
        self.composing_text = Some(String::new());
        self.composing_start = self.cursor_position;
    }

    pub fn update_composition(&mut self, text: String) {
        if self.composing_text.is_some() {
            self.composing_text = Some(text);
        }
    }

    pub fn finish_composition(&mut self) -> Option<String> {
        self.composing_text.take()
    }

    pub fn composing_text(&self) -> Option<&String> {
        self.composing_text.as_ref()
    }

    pub fn composing_start(&self) -> usize {
        self.composing_start
    }

    pub fn set_selection(&mut self, anchor: usize, end: usize) {
        self.selection_anchor = anchor;
        self.selection_end = end;
    }

    fn span_index_at(&self, byte_pos: usize) -> (usize, usize) {
        let mut offset = 0;
        for (i, span) in self.spans.iter().enumerate() {
            let end = offset + span.text.len();
            if byte_pos <= end {
                return (i, byte_pos - offset);
            }
            offset = end;
        }
        if self.spans.is_empty() {
            return (0, 0);
        }
        let last = self.spans.len() - 1;
        (last, self.spans[last].text.len())
    }

    fn insert_at(&mut self, byte_pos: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        if self.spans.is_empty() {
            self.spans.push(SpanData {
                text: text.to_string(),
                bold: false,
                italic: false,
                underline: false,
                font_size: None,
                color: None,
            });
            return;
        }
        let (idx, local_offset) = self.span_index_at(byte_pos);
        self.spans[idx].text.insert_str(local_offset, text);
    }

    pub fn delete_range(&mut self, start: usize, end: usize) {
        if start >= end || self.spans.is_empty() {
            return;
        }
        let total = self.full_len();
        let end = end.min(total);
        let start = start.min(total);
        if start >= end {
            return;
        }

        let (start_idx, start_local) = self.span_index_at(start);
        let (end_idx, end_local) = self.span_index_at(end);

        if start_idx == end_idx {
            self.spans[start_idx].text.replace_range(start_local..end_local, "");
        } else {
            self.spans[start_idx].text.truncate(start_local);
            self.spans[end_idx].text.replace_range(0..end_local, "");
            for i in (start_idx + 1..end_idx).rev() {
                self.spans.remove(i);
            }
        }

        self.spans.retain(|s| !s.text.is_empty());
        self.merge_adjacent();
    }

    fn merge_adjacent(&mut self) {
        if self.spans.len() <= 1 {
            return;
        }
        let mut i = 0;
        while i < self.spans.len() - 1 {
            let can_merge = {
                let a = &self.spans[i];
                let b = &self.spans[i + 1];
                a.bold == b.bold
                    && a.italic == b.italic
                    && a.underline == b.underline
                    && a.font_size == b.font_size
                    && a.color == b.color
            };
            if can_merge {
                let b_text = self.spans[i + 1].text.clone();
                self.spans[i].text.push_str(&b_text);
                self.spans.remove(i + 1);
            } else {
                i += 1;
            }
        }
    }
}

impl Default for TextEditingController {
    fn default() -> Self {
        Self::new()
    }
}
