use libc::c_int;
use std::{
    os::{raw::c_void, unix::io::AsRawFd},
    ptr,
};

use weechat_sys::{t_weechat_plugin, WEECHAT_RC_OK};

use super::Hook;
use crate::Weechat;

/// Setting for the FdHook.
pub enum FdHookMode {
    /// Catch read events.
    Read,
    /// Catch write events.
    Write,
    /// Catch read and write events.
    ReadWrite,
}

impl FdHookMode {
    pub(crate) fn as_tuple(&self) -> (i32, i32) {
        let read = match self {
            FdHookMode::Read => 1,
            FdHookMode::ReadWrite => 1,
            FdHookMode::Write => 0,
        };

        let write = match self {
            FdHookMode::Read => 0,
            FdHookMode::ReadWrite => 1,
            FdHookMode::Write => 1,
        };
        (read, write)
    }
}

/// Hook for a file descriptor, the hook is removed when the object is dropped.
pub struct FdHook<F> {
    _hook: Hook,
    _hook_data: Box<FdHookData<F>>,
}

/// Callback trait for file descriptor based hooks.
pub trait FdHookCallback {
    /// The concrete type of the hooked file descriptor object.
    type FdObject;
    /// The callback that will be called when data is available to be read or to
    /// be written on the file descriptor based object.
    ///
    /// # Arguments
    ///
    /// * `weechat` - A Weechat context.
    ///
    /// * `fd_object` - The file-descriptor based object that was registered to
    ///     be watched for reads or writes.
    fn callback(&mut self, weechat: &Weechat, fd_object: &mut Self::FdObject);
}

struct FdHookData<F> {
    callback: Box<dyn FdHookCallback<FdObject = F>>,
    weechat_ptr: *mut t_weechat_plugin,
    fd_object: F,
}

impl<F> FdHook<F> {
    /// Hook an object that can be turned into a raw file descriptor.
    /// Returns the hook object.
    ///
    /// # Arguments
    ///
    /// * `fd_object` - An object for wich the file descriptor will be watched
    ///     and the callback called when read or write operations can happen
    ///     on it.
    ///
    /// * `mode` - Configure the hook to watch for writes, reads or both on the
    ///     file descriptor.
    ///
    /// * `callback` - A function that will be called if a watched event on the
    ///     file descriptor happends.
    ///
    /// * `callback_data` - Data that will be passed to the callback every time
    ///     the callback runs. This data will be freed when the hook is
    ///     unhooked.
    ///
    /// # Panics
    ///
    /// Panics if the method is not called from the main Weechat thread.
    ///
    /// # Example
    ///
    /// ```no_run
    ///
    /// # use weechat::{Weechat, hooks::{FdHook, FdHookMode, FdHookCallback}};
    /// # use pipe_channel::{channel, Receiver, Sender};
    ///
    /// struct Data;
    ///
    /// impl FdHookCallback for Data {
    ///     type FdObject = Receiver<String>;
    ///
    ///     fn callback(&mut self, _: &Weechat, receiver: &mut Receiver<String>) {
    ///         if let Ok(data) = receiver.recv() {
    ///             Weechat::print(&data)
    ///         }
    ///     }
    /// }
    ///
    /// let (sender, receiver): (Sender<String>, Receiver<String>) = channel();
    ///
    /// let hook = FdHook::new(receiver, FdHookMode::Read, Data)
    ///     .expect("Can't create executor FD hook");
    ///
    /// ```
    pub fn new(
        fd_object: F,
        mode: FdHookMode,
        callback: impl FdHookCallback<FdObject = F> + 'static,
    ) -> Result<FdHook<F>, ()>
    where
        F: AsRawFd,
    {
        unsafe extern "C" fn c_hook_cb<F>(
            pointer: *const c_void,
            _data: *mut c_void,
            _fd: i32,
        ) -> c_int {
            let hook_data: &mut FdHookData<F> = { &mut *(pointer as *mut FdHookData<F>) };
            let cb = &mut hook_data.callback;
            let mut fd_object = &mut hook_data.fd_object;
            let weechat = Weechat::from_ptr(hook_data.weechat_ptr);

            cb.callback(&weechat, &mut fd_object);

            WEECHAT_RC_OK
        }

        Weechat::check_thread();
        let weechat = unsafe { Weechat::weechat() };

        let fd = fd_object.as_raw_fd();

        let data = Box::new(FdHookData {
            callback: Box::new(callback),
            weechat_ptr: weechat.ptr,
            fd_object,
        });

        let data_ref = Box::leak(data);
        let hook_fd = weechat.get().hook_fd.unwrap();
        let (read, write) = mode.as_tuple();

        let hook_ptr = unsafe {
            hook_fd(
                weechat.ptr,
                fd,
                read,
                write,
                0,
                Some(c_hook_cb::<F>),
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
            Ok(FdHook::<F> {
                _hook: hook,
                _hook_data: hook_data,
            })
        }
    }
}
