#![warn(missing_docs)]

//! Main weechat module

use weechat_sys::{
    t_weechat_plugin,
    t_gui_buffer,
    WEECHAT_RC_OK,
    WEECHAT_RC_ERROR
};
use std::ffi::{CStr, CString};
use libc::{c_char, c_int};
use std::os::raw::c_void;
use std::ptr;
use buffer::Buffer;
use hooks::{Hook, HookData, CommandInfo};

/// Main Weechat struct that encapsulates common weechat API functions.
/// It has a similar API as the weechat script API.
pub struct Weechat {
    ptr: *mut t_weechat_plugin,
}

struct BufferPointers<'a, A: 'a, B, C: 'a> {
    weechat: *mut t_weechat_plugin,
    input_cb: Option<fn(&Option<A>, &mut B, Buffer, &str)>,
    input_data: B,
    input_data_ref: &'a Option<A>,
    close_cb: Option<fn(&Option<C>, Buffer)>,
    close_cb_data: &'a Option<C>,
}

type WeechatInputCbT = unsafe extern "C" fn(
    pointer: *const c_void,
    data: *mut c_void,
    buffer: *mut t_gui_buffer,
    input_data: *const c_char
) -> c_int;

impl Weechat {
    /// Create a Weechat object from a C t_weechat_plugin pointer.
    /// * `ptr` - Pointer of the weechat plugin.
    pub fn from_ptr(ptr: *mut t_weechat_plugin) -> Weechat {
        assert!(!ptr.is_null());

        Weechat {
            ptr: ptr,
        }
    }
}

impl Weechat {
    #[inline]
    pub(crate) fn get(&self) -> &t_weechat_plugin {
        unsafe {
            &*self.ptr
        }
    }

    /// Write a message in WeeChat log file (weechat.log).
    pub fn log(&self, msg: &str) {
        let log_printf = self.get().log_printf.unwrap();

        let fmt = CString::new("%s").unwrap();
        let msg = CString::new(msg).unwrap();

        unsafe {
            log_printf(fmt.as_ptr(), msg.as_ptr());
        }
    }

    /// Create a new Weechat buffer
    /// * `name` - Name of the new buffer
    /// * `input_cb` - Callback that will be called when something is entered into the input bar of
    /// the buffer
    /// * `input_data_ref` - Reference to some arbitrary data that will be passed to the input
    /// callback
    /// * `input_data` - Data that will be taken over by weechat and passed to the input callback,
    /// this data will be freed when the buffer closes
    /// * `close_cb` - Callback that will be called when the buffer is closed.
    /// * `close_cb_data` - Reference to some data that will be passed to the close callback.
    pub fn buffer_new<A, B: Default, C>(
        &self,
        name: &str,
        input_cb: Option<fn(&Option<A>, &mut B, Buffer, &str)>,
        input_data_ref: &'static Option<A>,
        input_data: Option<B>,
        close_cb: Option<fn(&Option<C>, Buffer)>,
        close_cb_data: &'static Option<C>,
    ) -> Buffer {
        unsafe extern "C" fn c_input_cb<A, B, C>(
            pointer: *const c_void,
            _data: *mut c_void,
            buffer: *mut t_gui_buffer,
            input_data: *const c_char,
        ) -> c_int {
            let input_data = CStr::from_ptr(input_data).to_str();

            let pointers: &mut BufferPointers<A, B, C> =
                { &mut *(pointer as *mut BufferPointers<A, B, C>) };

            let input_data = match input_data {
                Ok(x) => x,
                Err(_) => return WEECHAT_RC_ERROR,
            };

            let buffer = Buffer::from_ptr(pointers.weechat, buffer);
            let data_ref = pointers.input_data_ref;
            let data = &mut pointers.input_data;

            match pointers.input_cb {
                Some(callback) => callback(data_ref, data, buffer, input_data),
                None => {}
            };

            WEECHAT_RC_OK
        }

        unsafe extern "C" fn c_close_cb<A, B, C>(
            pointer: *const c_void,
            _data: *mut c_void,
            buffer: *mut t_gui_buffer,
        ) -> c_int {
            // We use from_raw() here so that the box get's freed at the end of this scope.
            let pointers = Box::from_raw(pointer as *mut BufferPointers<A, B, C>);
            let buffer = Buffer::from_ptr(pointers.weechat, buffer);

            let data_ref = pointers.close_cb_data;

            match pointers.close_cb {
                Some(callback) => callback(data_ref, buffer),
                None => {}
            };
            WEECHAT_RC_OK
        }

        // We create a box and use leak to stop rust from freeing our data,
        // we are giving weechat ownership over the data and will free it in the buffer close
        // callback.
        let buffer_pointers = Box::new(BufferPointers::<A, B, C> {
            weechat: self.ptr,
            input_cb: input_cb,
            input_data: input_data.unwrap_or_default(),
            input_data_ref: input_data_ref,
            close_cb: close_cb,
            close_cb_data: close_cb_data,
        });
        let buffer_pointers_ref: &BufferPointers<A, B, C> = Box::leak(buffer_pointers);

        let buf_new = self.get().buffer_new.unwrap();
        let c_name = CString::new(name).unwrap();

        let c_input_cb: Option<WeechatInputCbT> = match input_cb {
                Some(_) => Some(c_input_cb::<A, B, C>),
                None => None
            };

        let buf_ptr = unsafe {
            buf_new(
                self.ptr,
                c_name.as_ptr(),
                c_input_cb,
                buffer_pointers_ref as *const _ as *const c_void,
                ptr::null_mut(),
                Some(c_close_cb::<A, B, C>),
                buffer_pointers_ref as *const _ as *const c_void,
                ptr::null_mut()
            )
        };

        let buffer_set = self.get().buffer_set.unwrap();
        let option = CString::new("nicklist").unwrap();
        let value = CString::new("1").unwrap();

        unsafe {
            buffer_set(buf_ptr, option.as_ptr(), value.as_ptr())
        };

        Buffer {
            weechat: self.ptr,
            ptr: buf_ptr
        }
    }

    /// Display a message on the core weechat buffer.
    pub fn print(&self, msg: &str) {
        let printf_date_tags = self.get().printf_date_tags.unwrap();

        let fmt = CString::new("%s").unwrap();
        let msg = CString::new(msg).unwrap();

        unsafe {
            printf_date_tags(ptr::null_mut(), 0, ptr::null(), fmt.as_ptr(), msg.as_ptr());
        }
    }

    /// Create a new weechat command. Returns the hook of the command. The command is unhooked if
    /// the hook is dropped.
    pub fn hook_command<T: Default>(
        &self,
        command_info: CommandInfo,
        callback: fn(data: &T, buffer: Buffer),
        callback_data: Option<T>
    ) -> Hook<T> {

        unsafe extern "C" fn c_hook_cb<T>(
            pointer: *const c_void,
            _data: *mut c_void,
            buffer: *mut t_gui_buffer,
            argc: i32,
            argv: *mut *mut c_char,
            _argv_eol: *mut *mut c_char,
        ) -> c_int {
            let hook_data: &mut HookData<T> =
                { &mut *(pointer as *mut HookData<T>) };
            let buffer = Buffer::from_ptr(hook_data.weechat_ptr, buffer);
            let callback = hook_data.callback;
            let callback_data = &hook_data.callback_data;

            callback(callback_data, buffer);

            WEECHAT_RC_OK
        }

        let name = CString::new(command_info.name).unwrap();
        let description = CString::new(command_info.description).unwrap();
        let args = CString::new(command_info.args).unwrap();
        let args_description = CString::new(command_info.args_description).unwrap();
        let completion = CString::new(command_info.completion).unwrap();

        let data = Box::new(
            HookData {
                callback: callback,
                callback_data: callback_data.unwrap_or_default(),
                weechat_ptr: self.ptr
            }
        );

        let data_ref = Box::leak(data);

        let hook_command = self.get().hook_command.unwrap();
        let hook_ptr = unsafe {
            hook_command(
                self.ptr,
                name.as_ptr(),
                description.as_ptr(),
                args.as_ptr(),
                args_description.as_ptr(),
                completion.as_ptr(),
                Some(c_hook_cb::<T>),
                data_ref as *const _ as *const c_void,
                ptr::null_mut(),
            )
        };
        let hook_data = unsafe { Box::from_raw(data_ref) };

        Hook::<T> { ptr: hook_ptr, weechat_ptr: self.ptr , _hook_data: hook_data}
    }
}