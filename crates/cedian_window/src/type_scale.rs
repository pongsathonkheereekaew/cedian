//! The type sizes (Interface, Type): each name sets a size and its line height together, so
//! a block of text is one of them and never a size of its own.

use gpui::{Styled, px};

pub trait TypeScale: Styled + Sized {
    /// Settings section titles.
    fn text_title(self) -> Self {
        self.text_size(px(22.)).line_height(px(30.))
    }

    /// The thread's title in the sheet's header.
    fn text_pane_title(self) -> Self {
        self.text_size(px(14.)).line_height(px(22.))
    }

    /// The agent's answers, in the serif.
    fn text_answer(self) -> Self {
        self.text_size(px(16.)).line_height(px(27.))
    }

    /// The person's text and the composer.
    fn text_message(self) -> Self {
        self.text_size(px(14.5)).line_height(px(24.))
    }

    /// Lists, buttons and inputs.
    fn text_ui(self) -> Self {
        self.text_size(px(13.5)).line_height(px(21.))
    }

    /// Chips and secondary lines.
    fn text_meta(self) -> Self {
        self.text_size(px(12.)).line_height(px(18.))
    }

    /// Group headers and badges.
    fn text_label(self) -> Self {
        self.text_size(px(11.5)).line_height(px(16.))
    }

    /// The editor, tool output and the terminal.
    fn text_code(self) -> Self {
        self.text_size(px(12.5)).line_height(px(20.))
    }
}

impl<T: Styled> TypeScale for T {}
