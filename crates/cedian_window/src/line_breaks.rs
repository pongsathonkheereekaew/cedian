//! Where a line of text may break. GPUI's wrapper breaks at spaces and punctuation, and Thai
//! puts no spaces between words, so it broke Thai mid-word, even between a consonant and its
//! vowel. Text with Thai in it is cut at the dictionary's word boundaries (ICU4X) and laid out
//! filled into lines one whole piece at a time; other text keeps GPUI's own wrapping.

use std::sync::LazyLock;

use std::{cell::RefCell, rc::Rc};

use gpui::{
    AnyElement, App, AvailableSpace, Bounds, Element, ElementId, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, ParentElement as _, Pixels, ShapedLine,
    SharedString, Style, TextAlign, TextStyle, Window, div, point, px, size,
};
use icu_segmenter::{LineSegmenter, LineSegmenterBorrowed, options::LineBreakOptions};

static SEGMENTER: LazyLock<LineSegmenterBorrowed<'static>> =
    LazyLock::new(|| LineSegmenter::new_dictionary(LineBreakOptions::default()));

pub fn has_thai(text: &str) -> bool {
    text.chars().any(|c| ('\u{0E00}'..='\u{0E7F}').contains(&c))
}

/// The text cut at every place a line may break; each piece keeps its trailing spaces.
pub fn pieces(line: &str) -> Vec<&str> {
    let breaks: Vec<usize> = SEGMENTER.segment_str(line).collect();
    let mut ranges: Vec<std::ops::Range<usize>> = Vec::new();
    for pair in breaks.windows(2) {
        let range = pair[0]..pair[1];
        match ranges.last_mut() {
            // The dictionary ends a Thai word before the space after it; the space stays with
            // the word, as it does after an English one.
            Some(last) if line[range.clone()].trim().is_empty() => last.end = range.end,
            _ if range.is_empty() => {}
            _ => ranges.push(range),
        }
    }
    ranges.into_iter().map(|range| &line[range]).collect()
}

/// The text as an element that wraps at word boundaries in every script.
pub fn wrapped(text: &str) -> AnyElement {
    if has_thai(text) {
        WordWrapped::new(text).into_any_element()
    } else {
        div().child(text.to_owned()).into_any_element()
    }
}

/// Text laid out line by line at the break points of `pieces`, then shaped one line at a time,
/// so GPUI's wrapper never sees a line too long for the width.
pub struct WordWrapped {
    text: SharedString,
    laid: Rc<RefCell<Option<Laid>>>,
}

struct Laid {
    lines: Vec<ShapedLine>,
    line_height: Pixels,
}

impl WordWrapped {
    pub fn new(text: &str) -> Self {
        Self {
            text: text.to_owned().into(),
            laid: Rc::default(),
        }
    }
}

impl Element for WordWrapped {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        _cx: &mut App,
    ) -> (LayoutId, ()) {
        let style = window.text_style();
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line_height = window.pixel_snap(
            style
                .line_height
                .to_pixels(font_size.into(), window.rem_size()),
        );
        let text = self.text.clone();
        let laid = self.laid.clone();
        let id = window.request_measured_layout(
            Style::default(),
            move |known, available, window, _cx| {
                let width = known.width.or(match available.width {
                    AvailableSpace::Definite(width) => Some(width),
                    _ => None,
                });
                let lines = lay_out(&text, &style, font_size, width, window);
                let size = size(
                    lines
                        .iter()
                        .map(ShapedLine::width)
                        .fold(px(0.), Pixels::max),
                    line_height * lines.len() as f32,
                );
                *laid.borrow_mut() = Some(Laid { lines, line_height });
                size
            },
        );
        (id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _: &mut (),
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let laid = self.laid.borrow();
        let Some(laid) = laid.as_ref() else {
            return;
        };
        for (row, line) in laid.lines.iter().enumerate() {
            let origin = bounds.origin + point(px(0.), laid.line_height * row as f32);
            if let Err(error) =
                line.paint(origin, laid.line_height, TextAlign::Left, None, window, cx)
            {
                log::warn!("painting text: {error:#}");
            }
        }
    }
}

impl IntoElement for WordWrapped {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

/// Fills each line with whole pieces until the next would pass `width`. A piece wider than
/// the width gets a line of its own.
fn lay_out(
    text: &str,
    style: &TextStyle,
    font_size: Pixels,
    width: Option<Pixels>,
    window: &mut Window,
) -> Vec<ShapedLine> {
    let system = window.text_system().clone();
    let shape = |line: &str| {
        let line = line.trim_end();
        system.shape_line(
            line.to_owned().into(),
            font_size,
            &[style.to_run(line.len())],
            None,
        )
    };
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let Some(width) = width else {
            lines.push(shape(paragraph));
            continue;
        };
        let mut start = 0;
        let mut end = 0;
        for piece in pieces(paragraph) {
            let piece_end = end + piece.len();
            if end > start && shape(&paragraph[start..piece_end]).width() > width {
                lines.push(shape(&paragraph[start..end]));
                start = end;
            }
            end = piece_end;
        }
        lines.push(shape(&paragraph[start..end]));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thai_breaks_between_words_not_inside_them() {
        let text = "ภาษาไทยไม่เว้นวรรคระหว่างคำ";
        let pieces = pieces(text);
        assert_eq!(pieces.concat(), text);
        assert!(pieces.len() >= 4, "{pieces:?}");
        for word in ["ภาษา", "ไทย", "ระหว่าง", "คำ"] {
            assert!(pieces.contains(&word), "{word} cut in {pieces:?}");
        }
    }

    #[test]
    fn mixed_text_keeps_spaces_with_the_word_before() {
        let pieces = pieces("รัน grep -q good ผ่าน");
        assert_eq!(pieces.concat(), "รัน grep -q good ผ่าน");
        assert!(pieces.contains(&"grep "), "{pieces:?}");
        assert!(!pieces.iter().any(|p| p.starts_with(' ')), "{pieces:?}");
    }

    #[test]
    fn only_thai_takes_the_slow_path() {
        assert!(has_thai("ok ครับ"));
        assert!(!has_thai("plain English, café"));
    }
}
