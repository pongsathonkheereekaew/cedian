//! What a button does when clicked.

use gpui::{App, ClickEvent, Window};

pub type OnClick = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;
