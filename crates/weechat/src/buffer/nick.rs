use std::{borrow::Cow, ffi::CStr, marker::PhantomData};

use crate::{buffer::Buffer, LossyCString, Weechat};
use weechat_sys::{t_gui_buffer, t_gui_nick, t_weechat_plugin};

/// Settings to create a new nick.
pub struct NickSettings<'a> {
    /// Name of the new nick.
    pub(crate) name: &'a str,
    /// Color for the nick.
    pub(crate) color: &'a str,
    /// Prefix that will be shown before the name.
    pub(crate) prefix: &'a str,
    /// Color of the prefix.
    pub(crate) prefix_color: &'a str,
    /// Should the nick be visible in the nicklist.
    pub(crate) visible: bool,
}

impl<'a> NickSettings<'a> {
    /// Create new empyt nick creation settings.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the new nick.
    pub fn new(name: &str) -> NickSettings {
        NickSettings {
            name,
            color: "",
            prefix: "",
            prefix_color: "",
            visible: true,
        }
    }

    /// Set the color of the nick.
    ///
    /// # Arguments
    ///
    /// * `color` - The color that the nick should have.
    pub fn set_color(mut self, color: &'a str) -> NickSettings<'a> {
        self.color = color;
        self
    }

    /// Set the prefix of the nick.
    ///
    /// # Arguments
    ///
    /// * `prefix` - The prefix displayed before the nick in the nicklist.
    pub fn set_prefix(mut self, prefix: &'a str) -> NickSettings<'a> {
        self.prefix = prefix;
        self
    }

    /// Set the color of the nick prefix.
    ///
    /// # Arguments
    ///
    /// * `prefix_color` - The color that the prefix should have.
    pub fn set_prefix_color(mut self, prefix_color: &'a str) -> NickSettings<'a> {
        self.prefix_color = prefix_color;
        self
    }

    /// Set the visibility of the nick.
    ///
    /// # Arguments
    ///
    /// * `visible` - Should the nick be visible in the nicklist, `true` if it
    ///     should be visible, false otherwise. Defaults to `true`.
    pub fn set_visible(mut self, visible: bool) -> NickSettings<'a> {
        self.visible = visible;
        self
    }
}

/// Weechat Nick type
pub struct Nick<'a> {
    pub(crate) ptr: *mut t_gui_nick,
    pub(crate) buf_ptr: *mut t_gui_buffer,
    pub(crate) weechat_ptr: *mut t_weechat_plugin,
    pub(crate) buffer: PhantomData<&'a Buffer<'a>>,
}

impl<'a> Nick<'a> {
    /// Get a Weechat object out of the nick.
    fn get_weechat(&self) -> Weechat {
        Weechat::from_ptr(self.weechat_ptr)
    }

    /// Get a string property of the nick.
    /// * `property` - The name of the property to get the value for, this can
    ///     be one of name, color, prefix or prefix_color. If a unknown
    ///     property is requested an empty string is returned.
    fn get_string(&self, property: &str) -> Option<Cow<str>> {
        let weechat = self.get_weechat();
        let get_string = weechat.get().nicklist_nick_get_string.unwrap();
        let c_property = LossyCString::new(property);
        unsafe {
            let ret = get_string(self.buf_ptr, self.ptr, c_property.as_ptr());

            if ret.is_null() {
                None
            } else {
                Some(CStr::from_ptr(ret).to_string_lossy())
            }
        }
    }

    /// Get the name property of the nick.
    pub fn name(&self) -> Cow<str> {
        self.get_string("name").unwrap()
    }

    /// Get the color of the nick.
    pub fn color(&self) -> Cow<str> {
        self.get_string("color").unwrap()
    }

    /// Get the prefix of the nick.
    pub fn prefix(&self) -> Cow<str> {
        self.get_string("prefix").unwrap()
    }

    /// Get the color of the nick prefix.
    pub fn prefix_color(&self) -> Cow<str> {
        self.get_string("prefix_color").unwrap()
    }
}
