//! Bar items are used to display status information in Weechat.
use core::ptr;
use libc::c_char;
use std::os::raw::c_void;
use weechat_sys::{t_gui_bar_item, t_gui_buffer, t_gui_window, t_hashtable, t_weechat_plugin};

use crate::{buffer::Buffer, LossyCString, Weechat};

/// Trait for the bar item callback
///
/// A blanket implementation for pure `FnMut` functions exists, if data needs to
/// be passed to the callback implement this over your struct.
pub trait BarItemCallback: 'static {
    /// The callback that should be called after the bar items
    /// is marked to be updated.
    ///
    /// Should return a string that will be displayed by the bar item.
    ///
    /// # Arguments
    ///
    /// * `weeechat` - A reference to the weechat context.
    ///
    /// * `buffer` - The currently visible buffer.
    fn callback(&mut self, weechat: &Weechat, buffer: &Buffer) -> String;
}

impl<T: FnMut(&Weechat, &Buffer) -> String + 'static> BarItemCallback for T {
    fn callback(&mut self, weechat: &Weechat, buffer: &Buffer) -> String {
        self(weechat, buffer)
    }
}

struct BarItemCbData {
    callback: Box<dyn BarItemCallback>,
    weechat_ptr: *mut t_weechat_plugin,
}

/// A handle to a bar item. The bar item is automatically removed when the object is
/// dropped.
pub struct BarItem {
    name: String,
    ptr: *mut t_gui_bar_item,
    weechat: *mut t_weechat_plugin,
    _data: Box<BarItemCbData>,
}

impl Drop for BarItem {
    fn drop(&mut self) {
        let weechat = Weechat::from_ptr(self.weechat);
        let bar_item_remove = weechat.get().bar_item_remove.unwrap();
        unsafe { bar_item_remove(self.ptr) };
    }
}

impl BarItem {
    /// Create a new bar item that can be added by a user.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the new bar item.
    ///
    /// * `callback` - The callback that should be called after the bar items
    /// is marked to be updated.
    ///
    /// # Panics
    ///
    /// Panics if the method is not called from the main Weechat thread.
    ///
    /// # Example
    /// ```no_run
    /// # use weechat::Weechat;
    /// # use weechat::buffer::Buffer;
    /// # use weechat::hooks::BarItem;
    /// let item = BarItem::new("buffer_plugin", |weechat:&Weechat,
    /// buffer: &Buffer| {
    ///     "rust/sample".to_owned()
    /// });
    /// ```
    ///
    // TODO: Provide window object, the callback should accept a Window object
    // wrapping a t_gui_window
    //
    // TODO: If we're going to allow bar items to be searched for like we do for
    // buffers, we need to do something about the multiple ownership that may
    // come from this.
    pub fn new(name: &str, callback: impl BarItemCallback) -> Result<BarItem, ()> {
        unsafe extern "C" fn c_item_cb(
            pointer: *const c_void,
            _data: *mut c_void,
            _bar_item: *mut t_gui_bar_item,
            _window: *mut t_gui_window,
            buffer: *mut t_gui_buffer,
            _extra_info: *mut t_hashtable,
        ) -> *mut c_char {
            let data: &mut BarItemCbData = { &mut *(pointer as *mut BarItemCbData) };
            let weechat = Weechat::from_ptr(data.weechat_ptr);
            let buffer = weechat.buffer_from_ptr(buffer);

            let cb_trait = &mut data.callback;

            let ret = cb_trait.callback(&weechat, &buffer);

            // Weechat wants a malloc'ed string
            libc::strdup(LossyCString::new(ret).as_ptr())
        }
        Weechat::check_thread();
        let weechat = unsafe { Weechat::weechat() };

        let data = Box::new(BarItemCbData {
            callback: Box::new(callback),
            weechat_ptr: weechat.ptr,
        });

        let data_ref = Box::leak(data);
        let bar_item_new = weechat.get().bar_item_new.unwrap();

        let bar_item_name = LossyCString::new(name);

        let bar_item_ptr = unsafe {
            bar_item_new(
                weechat.ptr,
                bar_item_name.as_ptr(),
                Some(c_item_cb),
                data_ref as *const _ as *const c_void,
                ptr::null_mut(),
            )
        };

        let cb_data = unsafe { Box::from_raw(data_ref) };

        if bar_item_ptr.is_null() {
            return Err(());
        }

        Ok(BarItem {
            name: name.to_owned(),
            ptr: bar_item_ptr,
            weechat: weechat.ptr,
            _data: cb_data,
        })
    }

    /// Update the content of the bar item, by calling its build callback.
    pub fn update(&self) {
        Weechat::bar_item_update(&self.name);
    }
}
