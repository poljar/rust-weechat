use std::{
    borrow::Cow,
    cell::Cell,
    ffi::CStr,
    os::raw::{c_char, c_int, c_void},
    ptr,
    rc::Rc,
};

use weechat_sys::{t_gui_buffer, t_weechat_plugin};

use super::Hook;
use crate::{
    buffer::{Buffer, InnerBuffer, InnerBuffers},
    LossyCString, ReturnCode, Weechat,
};

/// Hook for a signal, the hook is removed when the object is dropped.
pub struct SignalHook {
    _hook: Hook,
    _hook_data: Box<SignalHookData>,
}

struct SignalHookData {
    callback: Box<dyn SignalCallback>,
    weechat_ptr: *mut t_weechat_plugin,
}

/// Enum over the different data types a signal may send.
#[non_exhaustive]
pub enum SignalData<'a> {
    /// String data
    String(Cow<'a, str>),
    /// Integer data
    Integer(i32),
    /// Buffer that was sent with the signal.
    Buffer(Buffer<'a>),
}

impl<'a> Into<SignalData<'a>> for &'a str {
    fn into(self) -> SignalData<'a> {
        SignalData::String(Cow::from(self))
    }
}

impl<'a> Into<SignalData<'a>> for String {
    fn into(self) -> SignalData<'a> {
        SignalData::String(Cow::from(self))
    }
}

impl<'a> Into<SignalData<'a>> for i32 {
    fn into(self) -> SignalData<'a> {
        SignalData::Integer(self)
    }
}

impl<'a> Into<SignalData<'a>> for Buffer<'a> {
    fn into(self) -> SignalData<'a> {
        SignalData::Buffer(self)
    }
}

impl<'a> Into<SignalData<'a>> for &'a Buffer<'a> {
    fn into(self) -> SignalData<'a> {
        let ptr = self.ptr();

        SignalData::Buffer(Buffer {
            inner: InnerBuffers::BorrowedBuffer(InnerBuffer {
                ptr,
                weechat: self.inner.weechat(),
                closing: Rc::new(Cell::new(false)),
            }),
        })
    }
}

impl<'a> SignalData<'a> {
    fn pointer_is_buffer(signal_name: &str) -> bool {
        // This table is taken from the Weechat plugin API docs
        //
        // https://weechat.org/files/doc/stable/weechat_plugin_api.en.html#_hook_signal
        #[allow(clippy::match_like_matches_macro)]
        match signal_name {
            "irc_channel_opened" | "irc_pv_opened" | "irc_server_opened" => true,

            "logger_start" | "logger_stop" | "logger_backlog" => true,

            "spell_suggest" => true,

            "buffer_opened" | "buffer_closing" | "buffer_closed" | "buffer_cleared" => true,

            "buffer_filters_enabled"
            | "buffer_filters_disabled"
            | "buffer_hidden"
            | "buffer_unhidden" => true,

            "buffer_lines_hidden"
            | "buffer_localvar_added"
            | "buffer_localvar_changed"
            | "buffer_localvar_removed"
            | "buffer_merged"
            | "buffer_unmerged"
            | "buffer_moved"
            | "buffer_renamed"
            | "buffer_switch"
            | "buffer_title_changed"
            | "buffer_type_changed" => true,

            "buffer_zoomed" | "buffer_unzoomed" => true,

            "hotlist_changed" => true,

            "input_search" | "input_text_changed" | "input_text_cursor_moved" => true,

            // TODO nicklist group signals have a string representation of a
            // pointer concatenated to the group name

            // TODO some signals send out pointers to windows.
            // TODO some signals send out pointers to infolists.
            _ => false,
        }
    }

    fn from_type_and_name(
        weechat: &'a Weechat,
        signal_name: &str,
        data_type: &str,
        data: *mut c_void,
    ) -> Option<SignalData<'a>> {
        // Some signals don't send any data, some other signals might send out a
        // buffer pointer that might be null, in either case check if the
        // pointer is valid first.
        if data.is_null() {
            return None;
        }

        match data_type {
            "string" => unsafe {
                Some(SignalData::String(
                    CStr::from_ptr(data as *const c_char).to_string_lossy(),
                ))
            },
            "integer" => {
                let data = data as *const c_int;
                unsafe { Some(SignalData::Integer(*(data))) }
            }
            "pointer" => {
                if SignalData::pointer_is_buffer(signal_name) {
                    Some(SignalData::Buffer(
                        weechat.buffer_from_ptr(data as *mut t_gui_buffer),
                    ))
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

/// Trait for the signal callback.
///
/// A blanket implementation for pure `FnMut` functions exists, if data needs to
/// be passed to the callback implement this over your struct.
pub trait SignalCallback {
    /// Callback that will be called when a signal is fired.
    /// input field.
    ///
    /// # Arguments
    ///
    /// * `weechat` - A Weechat context.
    ///
    /// * `signal_name` - The name of the signal that fired the callback.
    ///
    /// * `data` - The data that was passed on by the signal.
    fn callback(
        &mut self,
        weechat: &Weechat,
        signal_name: &str,
        data: Option<SignalData>,
    ) -> ReturnCode;
}

impl<T: FnMut(&Weechat, &str, Option<SignalData>) -> ReturnCode + 'static> SignalCallback for T {
    fn callback(
        &mut self,
        weechat: &Weechat,
        signal_name: &str,
        data: Option<SignalData>,
    ) -> ReturnCode {
        self(weechat, signal_name, data)
    }
}

impl SignalHook {
    /// Hook a signal.
    ///
    /// # Arguments
    ///
    /// * `signal_name` - The signal to hook (wildcard `*` is allowed).
    ///
    /// * `callback` - A function or a struct that implements SignalCallback,
    /// the callback method of the trait will be called when the signal is
    /// fired.
    ///
    /// # Panics
    ///
    /// Panics if the method is not called from the main Weechat thread.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use weechat::{Weechat, ReturnCode};
    /// # use weechat::hooks::{SignalData, SignalHook};
    /// let signal_hook = SignalHook::new(
    ///     "buffer_switch",
    ///     |_weechat: &Weechat, _signal_name: &str, data: Option<SignalData>| {
    ///         if let Some(data) = data {
    ///             match data {
    ///                 SignalData::Buffer(buffer) => {
    ///                     buffer.print("Switched buffer")
    ///                 }
    ///                 _ => (),
    ///             }
    ///         }
    ///
    ///         ReturnCode::Ok
    ///     },
    /// );
    ///
    /// ```
    pub fn new(signal_name: &str, callback: impl SignalCallback + 'static) -> Result<Self, ()> {
        unsafe extern "C" fn c_hook_cb(
            pointer: *const c_void,
            _data: *mut c_void,
            signal_name: *const c_char,
            data_type: *const c_char,
            signal_data: *mut c_void,
        ) -> c_int {
            let hook_data: &mut SignalHookData = { &mut *(pointer as *mut SignalHookData) };
            let cb = &mut hook_data.callback;

            let data_type = CStr::from_ptr(data_type).to_str().unwrap_or_default();
            let signal_name = CStr::from_ptr(signal_name).to_str().unwrap_or_default();

            let weechat = Weechat::from_ptr(hook_data.weechat_ptr);
            let data =
                SignalData::from_type_and_name(&weechat, signal_name, data_type, signal_data);

            cb.callback(&weechat, signal_name, data) as i32
        }

        Weechat::check_thread();
        let weechat = unsafe { Weechat::weechat() };

        let data = Box::new(SignalHookData {
            callback: Box::new(callback),
            weechat_ptr: weechat.ptr,
        });

        let data_ref = Box::leak(data);
        let hook_signal = weechat.get().hook_signal.unwrap();

        let signal_name = LossyCString::new(signal_name);

        let hook_ptr = unsafe {
            hook_signal(
                weechat.ptr,
                signal_name.as_ptr(),
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
            Ok(SignalHook {
                _hook: hook,
                _hook_data: hook_data,
            })
        }
    }
}

impl Weechat {
    /// Send a signal.
    ///
    /// This will send out a signal and callbacks that are registered with a
    /// `SignalHook` to listen to that signal wil get called.
    ///
    /// # Arguments
    ///
    /// * `signal_name` - The name of the signal that should be sent out. Common
    ///     signals can be found in the Weechat plugin API [reference].
    ///
    /// * `data` - Data that should be provided to the signal callback. This can
    ///     be a string, an i32 number, or a buffer.
    ///
    /// ```no_run
    /// # use weechat::Weechat;
    /// # use weechat::buffer::BufferBuilder;
    /// # let buffer_handle = BufferBuilder::new("test")
    /// #    .build()
    /// #    .unwrap();
    /// # let buffer = buffer_handle.upgrade().unwrap();
    /// // Fetch the chat history for the buffer.
    /// Weechat::hook_signal_send("logger_backlog", &buffer);
    ///
    /// // Signal that the input text changed.
    /// Weechat::hook_signal_send("input_text_changed", "");
    /// ```
    ///
    /// [reference]: https://weechat.org/files/doc/stable/weechat_plugin_api.en.html#_hook_signal_send
    pub fn hook_signal_send<'a, D: Into<SignalData<'a>>>(signal_name: &str, data: D) -> ReturnCode {
        Weechat::check_thread();
        let weechat = unsafe { Weechat::weechat() };

        let signal_name = LossyCString::new(signal_name);
        let signal_send = weechat.get().hook_signal_send.unwrap();
        let data = data.into();

        let ret = if let SignalData::String(string) = data {
            let string = LossyCString::new(string);
            unsafe {
                signal_send(
                    signal_name.as_ptr(),
                    weechat_sys::WEECHAT_HOOK_SIGNAL_STRING as *const _ as *const c_char,
                    string.as_ptr() as *mut _,
                )
            }
        } else {
            let (ptr, data_type) = match data {
                SignalData::Integer(number) => (
                    number as *mut _,
                    weechat_sys::WEECHAT_HOOK_SIGNAL_INT as *const u8,
                ),
                SignalData::Buffer(buffer) => (
                    buffer.ptr() as *mut _,
                    weechat_sys::WEECHAT_HOOK_SIGNAL_POINTER as *const u8,
                ),
                SignalData::String(_) => unreachable!(),
            };
            unsafe { signal_send(signal_name.as_ptr(), data_type as *const c_char, ptr) }
        };

        match ret {
            weechat_sys::WEECHAT_RC_OK => ReturnCode::Ok,
            weechat_sys::WEECHAT_RC_OK_EAT => ReturnCode::OkEat,
            weechat_sys::WEECHAT_RC_ERROR => ReturnCode::Error,
            _ => ReturnCode::Error,
        }
    }
}
