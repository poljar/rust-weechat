use libc::{c_char, c_int};
use std::{borrow::Cow, ffi::CStr, os::raw::c_void, ptr};

use weechat_sys::{
    t_gui_buffer, t_gui_completion, t_weechat_plugin, WEECHAT_RC_ERROR, WEECHAT_RC_OK,
};

use crate::{buffer::Buffer, hooks::Hook, LossyCString, Weechat};

/// A handle to a completion item.
pub struct Completion {
    weechat_ptr: *mut t_weechat_plugin,
    ptr: *mut t_gui_completion,
}

/// Trait for the completion callback.
///
/// A blanket implementation for pure `FnMut` functions exists, if data needs to
pub trait CompletionCallback {
    /// Callback that will be called if when a completion is requested.
    ///
    /// # Arguments
    ///
    /// * `weechat` - A Weechat context.
    ///
    /// * `buffer` - The currently active buffer that requested the completion
    /// to run.
    ///
    /// * `completion_name` - The name of the completion.
    ///
    /// * `completion` - The completion object that should be populated with
    /// completion words by the callback.
    fn callback(
        &mut self,
        weechat: &Weechat,
        buffer: &Buffer,
        completion_name: Cow<str>,
        completion: &Completion,
    ) -> Result<(), ()>;
}

impl<T: FnMut(&Weechat, &Buffer, Cow<str>, &Completion) -> Result<(), ()> + 'static>
    CompletionCallback for T
{
    fn callback(
        &mut self,
        weechat: &Weechat,
        buffer: &Buffer,
        completion_name: Cow<str>,
        completion: &Completion,
    ) -> Result<(), ()> {
        self(weechat, buffer, completion_name, completion)
    }
}

/// The positions an entry can be added to a completion list.
#[derive(Clone, Copy)]
pub enum CompletionPosition {
    /// Insert the item in a way that keeps the list sorted.
    Sorted,
    /// Insert the item at the beginning of the list.
    Beginning,
    /// Insert the item at the end of the list.
    End,
}

impl CompletionPosition {
    pub(crate) fn value(&self) -> &str {
        match self {
            CompletionPosition::Sorted => "sort",
            CompletionPosition::Beginning => "beginning",
            CompletionPosition::End => "end",
        }
    }
}

impl Completion {
    pub(crate) fn from_raw(
        weechat: *mut t_weechat_plugin,
        completion: *mut t_gui_completion,
    ) -> Completion {
        Completion {
            weechat_ptr: weechat,
            ptr: completion,
        }
    }

    /// Add a word for completion, keeping the list sorted.
    pub fn add(&self, word: &str) {
        self.add_with_options(word, false, CompletionPosition::Sorted)
    }

    /// Get the command used in the completion.
    pub fn base_command(&self) -> Option<Cow<str>> {
        self.get_string("base_command")
    }

    /// Get the word that is being completed.
    pub fn base_word(&self) -> Option<Cow<str>> {
        self.get_string("base_word")
    }

    /// Get the command arguments including the base word.
    pub fn arguments(&self) -> Option<Cow<str>> {
        self.get_string("args")
    }

    fn get_string(&self, property_name: &str) -> Option<Cow<str>> {
        let weechat = Weechat::from_ptr(self.weechat_ptr);

        let get_string = weechat.get().hook_completion_get_string.unwrap();

        let property_name = LossyCString::new(property_name);

        unsafe {
            let ret = get_string(self.ptr, property_name.as_ptr());

            if ret.is_null() {
                None
            } else {
                Some(CStr::from_ptr(ret).to_string_lossy())
            }
        }
    }

    /// Add a word to the completion giving the position and wether the word is
    /// a nick.
    ///
    /// # Arguments
    ///
    /// * `word` - The word that should be added to the completion.
    ///
    /// * `is_nick` - Set if the word is a nick.
    ///
    /// * `position` - Set the position where the nick should be added to.
    pub fn add_with_options(&self, word: &str, is_nick: bool, position: CompletionPosition) {
        let weechat = Weechat::from_ptr(self.weechat_ptr);

        let hook_completion_list_add = weechat.get().hook_completion_list_add.unwrap();

        let word = LossyCString::new(word);
        let method = LossyCString::new(position.value());

        unsafe {
            hook_completion_list_add(self.ptr, word.as_ptr(), is_nick as i32, method.as_ptr());
        }
    }
}

/// Hook for a completion item, the hook is removed when the object is dropped.
pub struct CompletionHook {
    _hook: Hook,
    _hook_data: Box<CompletionHookData>,
}

struct CompletionHookData {
    #[allow(clippy::type_complexity)]
    callback: Box<dyn CompletionCallback>,
    weechat_ptr: *mut t_weechat_plugin,
}

impl CompletionHook {
    /// Create a new completion
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the new completion. After this is created the
    ///     can be used as `%(name)` when creating commands.
    ///
    /// * `description` - The description of the new completion.
    ///
    /// * `callback` - A function that will be called when the completion is
    ///     used, the callback must populate the candidates for the completion.
    ///
    /// # Example
    /// ```no_run
    /// # use std::borrow::Cow;
    /// # use std::collections::HashSet;
    /// # use weechat::Weechat;
    /// # use weechat::buffer::Buffer;
    /// # use weechat::hooks::{CompletionCallback, CompletionHook, Completion, CompletionPosition};
    ///
    /// pub struct MyMap {
    ///     server_names: HashSet<String>,
    /// }
    ///
    /// impl CompletionCallback for MyMap {
    ///     fn callback(&mut self, _: &Weechat, _: &Buffer, _: Cow<str>, completion: &Completion) -> Result<(), ()> {
    ///         for server_name in &self.server_names {
    ///             completion.add_with_options(server_name, false, CompletionPosition::Sorted);
    ///         }
    ///         Ok(())
    ///     }
    /// }
    ///
    /// let servers = MyMap { server_names: HashSet::new() };
    ///
    /// let completion = CompletionHook::new(
    ///     "matrix_servers",
    ///     "Completion for the list of added Matrix servers",
    ///     servers,
    /// ).unwrap();
    ///
    /// ```
    pub fn new(
        completion_item: &str,
        description: &str,
        callback: impl CompletionCallback + 'static,
    ) -> Result<CompletionHook, ()> {
        unsafe extern "C" fn c_hook_cb(
            pointer: *const c_void,
            _data: *mut c_void,
            completion_item: *const c_char,
            buffer: *mut t_gui_buffer,
            completion: *mut t_gui_completion,
        ) -> c_int {
            let hook_data: &mut CompletionHookData = { &mut *(pointer as *mut CompletionHookData) };
            let cb = &mut hook_data.callback;
            let weechat = Weechat::from_ptr(hook_data.weechat_ptr);
            let buffer = weechat.buffer_from_ptr(buffer);

            let completion_item = CStr::from_ptr(completion_item).to_string_lossy();

            let ret = cb.callback(
                &weechat,
                &buffer,
                completion_item,
                &Completion::from_raw(hook_data.weechat_ptr, completion),
            );

            if let Ok(()) = ret {
                WEECHAT_RC_OK
            } else {
                WEECHAT_RC_ERROR
            }
        }

        Weechat::check_thread();
        let weechat = unsafe { Weechat::weechat() };

        let data = Box::new(CompletionHookData {
            callback: Box::new(callback),
            weechat_ptr: weechat.ptr,
        });

        let data_ref = Box::leak(data);
        let hook_completion = weechat.get().hook_completion.unwrap();

        let completion_item = LossyCString::new(completion_item);
        let description = LossyCString::new(description);

        let hook_ptr = unsafe {
            hook_completion(
                weechat.ptr,
                completion_item.as_ptr(),
                description.as_ptr(),
                Some(c_hook_cb),
                data_ref as *const _ as *const c_void,
                ptr::null_mut(),
            )
        };

        let hook_data = unsafe { Box::from_raw(data_ref) };

        if hook_ptr.is_null() {
            return Err(());
        }

        let hook = Hook {
            ptr: hook_ptr,
            weechat_ptr: weechat.ptr,
        };

        Ok(CompletionHook {
            _hook: hook,
            _hook_data: hook_data,
        })
    }
}
