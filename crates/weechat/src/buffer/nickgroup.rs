use std::{borrow::Cow, ffi::CStr, marker::PhantomData};

use weechat_sys::{t_gui_buffer, t_gui_nick_group, t_weechat_plugin};

use crate::{
    buffer::{Buffer, Nick, NickSettings},
    LossyCString, Weechat,
};

/// Weechat nicklist Group type.
pub struct NickGroup<'a> {
    pub(crate) ptr: *mut t_gui_nick_group,
    pub(crate) buf_ptr: *mut t_gui_buffer,
    pub(crate) weechat_ptr: *mut t_weechat_plugin,
    pub(crate) buffer: PhantomData<&'a Buffer<'a>>,
}

impl<'a> NickGroup<'a> {
    fn get_weechat(&self) -> Weechat {
        Weechat::from_ptr(self.weechat_ptr)
    }

    /// Get a string property of the nick.
    /// * `property` - The name of the property to get the value for, this can
    ///     be one of name, color, prefix or prefix_color. If a unknown
    ///     property is requested an empty string is returned.
    fn get_string(&self, property: &str) -> Option<Cow<str>> {
        let weechat = self.get_weechat();
        let get_string = weechat.get().nicklist_group_get_string.unwrap();
        let c_property = LossyCString::new(property);

        let ret = unsafe { get_string(self.buf_ptr, self.ptr, c_property.as_ptr()) };

        if ret.is_null() {
            None
        } else {
            unsafe { Some(CStr::from_ptr(ret).to_string_lossy()) }
        }
    }

    fn get_integer(&self, property: &str) -> i32 {
        let weechat = self.get_weechat();
        let get_integer = weechat.get().nicklist_group_get_integer.unwrap();
        let c_property = LossyCString::new(property);

        unsafe { get_integer(self.buf_ptr, self.ptr, c_property.as_ptr()) }
    }

    /// Get the name of the group.
    pub fn name(&self) -> Cow<str> {
        self.get_string("name").unwrap()
    }

    /// Get the color of the group.
    pub fn color(&self) -> Cow<str> {
        self.get_string("color").unwrap()
    }

    /// Is the nick group visible.
    pub fn visible(&self) -> bool {
        self.get_integer("visible") != 0
    }

    /// Get the group nesting level.
    ///
    /// Returns 0 if this is the root group, 1 if it's a child of the root
    /// group.
    pub fn level(&self) -> u32 {
        self.get_integer("level") as u32
    }

    /// Create and add a new nick to the buffer nicklist under this group.
    ///
    /// # Arguments
    ///
    /// * `nick_settings` - Nick arguments struct for the nick that should be
    ///     added.
    ///
    /// Returns the newly created nick if one is created successfully, an empty
    /// error otherwise.
    pub fn add_nick(&self, nick_settings: NickSettings) -> Result<Nick, ()> {
        let weechat = self.get_weechat();
        let nick_ptr = Buffer::add_nick_helper(&weechat, self.buf_ptr, nick_settings, Some(self));

        if nick_ptr.is_null() {
            return Err(());
        }

        Ok(Nick {
            ptr: nick_ptr,
            buf_ptr: self.buf_ptr,
            weechat_ptr: self.get_weechat().ptr,
            buffer: PhantomData,
        })
    }

    /// Search for a nick in this nick group.
    ///
    /// # Arguments
    ///
    /// * `nick` - The name of the nick that should be found.
    ///
    /// Returns a `Nick` if one is found, None otherwise.
    pub fn search_nick(&self, nick: &str) -> Option<Nick> {
        let weechat = self.get_weechat();
        let nick = Buffer::search_nick_helper(&weechat, self.buf_ptr, nick, None);

        if nick.is_null() {
            None
        } else {
            Some(Nick {
                ptr: nick,
                buf_ptr: self.buf_ptr,
                weechat_ptr: weechat.ptr,
                buffer: PhantomData,
            })
        }
    }
}
