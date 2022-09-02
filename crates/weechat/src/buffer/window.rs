use std::{marker::PhantomData, ptr};

use weechat_sys::{t_gui_window, t_weechat_plugin};

use super::Buffer;
use crate::{LossyCString, Weechat};

/// A Weechat window.
///
/// A window is a screen area which displays a buffer. It is possible to split
/// your screen into many windows.
pub struct Window<'a> {
    pub(crate) weechat: *mut t_weechat_plugin,
    pub(crate) ptr: *mut t_gui_window,
    pub(crate) phantom: PhantomData<&'a Buffer<'a>>,
}

impl<'a> Window<'a> {
    fn get_integer(&self, property: &str) -> i32 {
        let weechat = Weechat::from_ptr(self.weechat);
        let get_integer = weechat.get().window_get_integer.unwrap();
        let property = LossyCString::new(property);

        unsafe { get_integer(self.ptr, property.as_ptr()) }
    }

    fn get_bool(&self, property: &str) -> bool {
        self.get_integer(property) == 1
    }

    /// The number of the window.
    pub fn number(&self) -> i32 {
        self.get_integer("number")
    }

    /// The X coordinate position of the window in the terminal (the first
    /// column is 0).
    pub fn x(&self) -> i32 {
        self.get_integer("win_x")
    }

    /// The Y coordinate position of the window in the terminal (the first
    /// line is 0).
    pub fn y(&self) -> i32 {
        self.get_integer("win_y")
    }

    /// The width of the window in chars.
    pub fn width(&self) -> i32 {
        self.get_integer("win_width")
    }

    /// The height of the window in chars.
    pub fn height(&self) -> i32 {
        self.get_integer("win_height")
    }

    /// The width of the window expressed as a percentage of the parent window,
    /// for example 50 means that the window is half of the size of the parent
    /// window.
    pub fn width_percentage(&self) -> i32 {
        self.get_integer("win_width_pct")
    }

    /// The height of the window expressed as a percentage of the parent window,
    /// for example 50 means that the window is half of the size of the parent
    /// window.
    pub fn height_percentage(&self) -> i32 {
        self.get_integer("win_height_pct")
    }

    /// The X coordinate position of the chat window in the terminal (the first
    /// column is 0).
    pub fn chat_x(&self) -> i32 {
        self.get_integer("win_chat_x")
    }

    /// The Y coordinate position of the chat window in the terminal (the first
    /// line is 0).
    pub fn chat_y(&self) -> i32 {
        self.get_integer("win_chat_y")
    }

    /// The width of the chat window in chars.
    pub fn chat_width(&self) -> i32 {
        self.get_integer("win_chat_width")
    }

    /// The height of the chat window in chars.
    pub fn chat_height(&self) -> i32 {
        self.get_integer("win_chat_height")
    }

    /// Returns true if the first line of the buffer is shown in the window, or
    /// to put it differently if the window is scrolled completely up.
    pub fn is_first_line_displayed(&self) -> bool {
        self.get_bool("first_line_displayed")
    }

    /// Returns true if the last line of the buffer is shown in the window, or
    /// to put it differently if the window is scrolled completely down.
    pub fn is_last_line_displayed(&self) -> bool {
        self.get_bool("scrolling")
    }

    /// This gives the number of lines that are not displayed towards the bottom
    /// of the buffer.
    pub fn lines_after(&self) -> i32 {
        self.get_integer("lines_after")
    }

    fn set_title_helper(&self, title: Option<&str>) {
        let weechat = Weechat::from_ptr(self.weechat);
        let set_title = weechat.get().window_set_title.unwrap();

        if let Some(title) = title {
            let title = LossyCString::new(title);
            unsafe {
                set_title(title.as_ptr());
            }
        } else {
            unsafe {
                set_title(ptr::null_mut());
            }
        };
    }

    /// Set the title for the terminal.
    ///
    /// # Arguments
    ///
    /// * `title` - The new title that should be set for the terminal, the
    /// string is evaluated, so variables like ${info:version} can be used.
    pub fn set_title(&self, title: &str) {
        self.set_title_helper(Some(title));
    }

    /// Reset the title for the terminal.
    pub fn reset_title(&self) {
        self.set_title_helper(None);
    }
}
