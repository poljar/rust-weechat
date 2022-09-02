use libc::c_char;
use std::{borrow::Cow, ffi::CStr, os::raw::c_void, ptr};

use weechat_sys::{t_gui_buffer, t_weechat_plugin};

use super::Hook;
use crate::{buffer::Buffer, LossyCString, Weechat};

/// Hook for a modifier, the hook is removed when the object is dropped.
#[cfg_attr(feature = "docs", doc(cfg(unsound)))]
pub struct ModifierHook {
    _hook: Hook,
    _hook_data: Box<ModifierHookData>,
}

struct ModifierHookData {
    callback: Box<dyn ModifierCallback>,
    weechat_ptr: *mut t_weechat_plugin,
}

/// Enum over the different data types a modifier may send.
pub enum ModifierData<'a> {
    /// String data
    String(Cow<'a, str>),
    /// Buffer that was sent with the modifier.
    Buffer(Buffer<'a>),
}

impl<'a> ModifierData<'a> {
    fn pointer_is_buffer(modifier_name: &str) -> bool {
        // This table is taken from the Weechat plugin API docs
        //
        // https://weechat.org/files/doc/stable/weechat_plugin_api.en.html#_hook_modifier
        if modifier_name.starts_with("bar_condition_") {
            true
        } else {
            matches!(
                modifier_name,
                "bar_condition_yyy"
                    | "history_add"
                    | "input_text_content"
                    | "input_text_display"
                    | "input_text_display_with_cursor"
                    | "input_text_for_buffer"
            )
        }
    }

    fn from_name(
        weechat: &'a Weechat,
        modifier_name: &str,
        data: *const c_char,
    ) -> Option<ModifierData<'a>> {
        if data.is_null() {
            return None;
        }

        let modifier_data = unsafe { CStr::from_ptr(data).to_string_lossy() };

        // Some modifier send out a buffer pointer converted to a string,
        // convert those to a buffer.
        if ModifierData::pointer_is_buffer(modifier_name) {
            if modifier_data.len() < 2 || !modifier_data.starts_with("0x") {
                None
            } else {
                let ptr = u64::from_str_radix(&modifier_data[2..], 16).ok()?;

                Some(ModifierData::Buffer(
                    weechat.buffer_from_ptr(ptr as *mut t_gui_buffer),
                ))
            }
        } else {
            Some(ModifierData::String(modifier_data))
        }
    }
}

/// Trait for the modifier callback.
///
/// A blanket implementation for pure `FnMut` functions exists, if data needs to
/// be passed to the callback implement this over your struct.
pub trait ModifierCallback {
    /// Callback that will be called when a modifier is fired.
    ///
    /// # Arguments
    ///
    /// * `weechat` - A Weechat context.
    ///
    /// * `modifier_name` - The name of the modifier that fired the callback.
    ///
    /// * `data` - The data that was passed on by the modifier.
    ///
    /// * `string` - The string that should be modified.
    fn callback(
        &mut self,
        weechat: &Weechat,
        modifier_name: &str,
        data: Option<ModifierData>,
        string: Cow<str>,
    ) -> Option<String>;
}

impl<T: FnMut(&Weechat, &str, Option<ModifierData>, Cow<str>) -> Option<String> + 'static>
    ModifierCallback for T
{
    /// Callback that will be called when a modifier is fired.
    ///
    /// # Arguments
    ///
    /// * `weechat` - A Weechat context.
    ///
    /// * `modifier_name` - The name of the modifier that fired the callback.
    ///
    /// * `data` - The data that was passed on by the modifier.
    ///
    /// * `string` - The string that should be modified.
    fn callback(
        &mut self,
        weechat: &Weechat,
        modifier_name: &str,
        data: Option<ModifierData>,
        string: Cow<str>,
    ) -> Option<String> {
        self(weechat, modifier_name, data, string)
    }
}

impl ModifierHook {
    /// Hook a modifier.
    ///
    /// # Arguments
    ///
    /// * `modifier_name` - The modifier to hook.
    ///
    /// * `callback` - A function or a struct that implements ModifierCallback,
    /// the callback method of the trait will be called when the modifier is
    /// fired.
    ///
    /// # Panics
    ///
    /// Panics if the method is not called from the main Weechat thread.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use std::borrow::Cow;
    /// # use weechat::{Weechat, ReturnCode};
    /// # use weechat::hooks::{ModifierData, ModifierHook};
    ///
    /// let modifier_hook = ModifierHook::new(
    /// "input_text_display_with_cursor",
    /// |_weechat: &Weechat,
    /// _modifier_name: &str,
    /// data: Option<ModifierData>,
    /// string: Cow<str>| {
    ///     if let ModifierData::Buffer(buffer) = data? {
    ///         buffer.print("Modifying the input buffer")
    ///     }
    ///
    ///     None
    /// });
    /// ```
    #[cfg_attr(feature = "docs", doc(cfg(unsound)))]
    pub fn new(modifier_name: &str, callback: impl ModifierCallback + 'static) -> Result<Self, ()> {
        unsafe extern "C" fn c_hook_cb(
            pointer: *const c_void,
            _data: *mut c_void,
            modifier_name: *const c_char,
            modifier_data: *const c_char,
            string: *const c_char,
        ) -> *mut c_char {
            let hook_data: &mut ModifierHookData = { &mut *(pointer as *mut ModifierHookData) };
            let cb = &mut hook_data.callback;

            let modifier_name = CStr::from_ptr(modifier_name).to_str().unwrap_or_default();

            let string = if string.is_null() {
                Cow::from("")
            } else {
                CStr::from_ptr(string).to_string_lossy()
            };

            let weechat = Weechat::from_ptr(hook_data.weechat_ptr);

            let data = ModifierData::from_name(&weechat, modifier_name, modifier_data);

            let modified_string = cb.callback(&weechat, modifier_name, data, string);

            if let Some(modified_string) = modified_string {
                let string_length = modified_string.len();
                let modified_string = LossyCString::new(modified_string);

                let strndup = weechat.get().strndup.unwrap();
                strndup(modified_string.as_ptr(), string_length as i32)
            } else {
                ptr::null_mut()
            }
        }

        Weechat::check_thread();
        let weechat = unsafe { Weechat::weechat() };

        let data = Box::new(ModifierHookData {
            callback: Box::new(callback),
            weechat_ptr: weechat.ptr,
        });

        let data_ref = Box::leak(data);
        let hook_modifier = weechat.get().hook_modifier.unwrap();

        let modifier_name = LossyCString::new(modifier_name);

        let hook_ptr = unsafe {
            hook_modifier(
                weechat.ptr,
                modifier_name.as_ptr(),
                Some(c_hook_cb),
                data_ref as *const _ as *const c_void,
                ptr::null_mut(),
            )
        };

        let hook_data = unsafe { Box::from_raw(data_ref) };
        let hook = Hook {
            ptr: hook_ptr,
            weechat_ptr: weechat.ptr,
        };

        if hook_ptr.is_null() {
            Err(())
        } else {
            Ok(Self {
                _hook: hook,
                _hook_data: hook_data,
            })
        }
    }
}
