//! Weechat Buffer module containing Buffer and Nick types.

mod lines;
mod nick;
mod nickgroup;
mod window;

use std::{
    borrow::Cow,
    cmp::{Ord, Ordering},
    ffi::{c_void, CStr},
    marker::PhantomData,
    ptr,
};

use std::{cell::Cell, rc::Rc};

#[cfg(feature = "async")]
use async_trait::async_trait;
#[cfg(feature = "async")]
use futures::future::LocalBoxFuture;

use crate::{LossyCString, Weechat};
use libc::{c_char, c_int};
use weechat_sys::{
    t_gui_buffer, t_gui_nick, t_hdata, t_weechat_plugin, WEECHAT_RC_ERROR, WEECHAT_RC_OK,
};

pub use crate::buffer::{
    lines::{BufferLine, BufferLines, LineData},
    nick::{Nick, NickSettings},
    nickgroup::NickGroup,
    window::Window,
};

/// A Weechat buffer.
///
/// A buffer contains the data displayed on the screen.
pub struct Buffer<'a> {
    pub(crate) inner: InnerBuffers<'a>,
}

impl<'a> std::fmt::Debug for Buffer<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Buffer")
            .field("full_name", &self.full_name())
            .finish()
    }
}

pub(crate) enum InnerBuffers<'a> {
    BorrowedBuffer(InnerBuffer<'a>),
    OwnedBuffer(InnerOwnedBuffer<'a>),
}

impl<'a> InnerBuffers<'a> {
    fn is_closing(&self) -> bool {
        match self {
            InnerBuffers::BorrowedBuffer(b) => b.closing.get(),
            InnerBuffers::OwnedBuffer(b) => b.closing.get(),
        }
    }

    fn mark_as_closing(&self) {
        match self {
            InnerBuffers::BorrowedBuffer(b) => b.closing.as_ref().replace(true),
            InnerBuffers::OwnedBuffer(b) => b.closing.as_ref().replace(true),
        };
    }
}

impl<'a> InnerBuffers<'a> {
    pub(crate) fn weechat(&self) -> &Weechat {
        match self {
            InnerBuffers::BorrowedBuffer(b) => &b.weechat,
            InnerBuffers::OwnedBuffer(b) => &b.weechat,
        }
    }
}

pub(crate) struct InnerOwnedBuffer<'a> {
    pub(crate) weechat: Weechat,
    pub(crate) buffer_handle: &'a BufferHandle,
    closing: Rc<Cell<bool>>,
}

pub(crate) struct InnerBuffer<'a> {
    pub(crate) weechat: &'a Weechat,
    pub(crate) ptr: *mut t_gui_buffer,
    pub(crate) closing: Rc<Cell<bool>>,
}

impl PartialEq for Buffer<'_> {
    fn eq(&self, other: &Buffer) -> bool {
        self.ptr() == other.ptr()
    }
}

impl PartialOrd for Buffer<'_> {
    fn partial_cmp(&self, other: &Buffer) -> Option<Ordering> {
        self.number().partial_cmp(&other.number())
    }
}

impl Eq for Buffer<'_> {}

impl Ord for Buffer<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.number().cmp(&other.number())
    }
}

/// A handle to a buffer that was created in the current plugin.
///
/// This means that the plugin owns this buffer. Nevertheless Weechat can
/// invalidate the buffer between callbacks at any point in time.
///
/// The buffer handle can be upgraded to a buffer which can then manipulate the
/// buffer state using the `upgrade()` method.
///
/// The buffer won't be closed if this handle gets dropped, to close the buffer
/// call the close method on the upgraded buffer object.
#[derive(Clone)]
pub struct BufferHandle {
    buffer_name: Rc<String>,
    weechat: *mut t_weechat_plugin,
    buffer_ptr: Rc<Cell<*mut t_gui_buffer>>,
    closing: Rc<Cell<bool>>,
}

impl BufferHandle {
    /// Upgrade the buffer handle into a `Buffer`.
    ///
    /// This is necessary to do because the handle can be invalidated by Weechat
    /// between callbacks.
    pub fn upgrade(&self) -> Result<Buffer<'_>, ()> {
        let ptr = self.buffer_ptr.get();

        if ptr.is_null() {
            Err(())
        } else {
            let buffer = Buffer {
                inner: InnerBuffers::OwnedBuffer(InnerOwnedBuffer {
                    weechat: Weechat::from_ptr(self.weechat),
                    buffer_handle: self,
                    closing: self.closing.clone(),
                }),
            };
            Ok(buffer)
        }
    }
}

#[cfg(feature = "async")]
pub(crate) struct BufferPointersAsync {
    pub(crate) weechat: *mut t_weechat_plugin,
    pub(crate) input_cb: Option<Box<dyn BufferInputCallbackAsync>>,
    pub(crate) close_cb: Option<Box<dyn BufferCloseCallback>>,
    pub(crate) buffer_cell: Option<Rc<Cell<*mut t_gui_buffer>>>,
}

pub(crate) struct BufferPointers {
    pub(crate) weechat: *mut t_weechat_plugin,
    pub(crate) input_cb: Option<Box<dyn BufferInputCallback>>,
    pub(crate) close_cb: Option<Box<dyn BufferCloseCallback>>,
    pub(crate) buffer_cell: Option<Rc<Cell<*mut t_gui_buffer>>>,
}

/// Trait for the buffer input callback
///
/// This is the sync version of the callback.
///
/// A blanket implementation for pure `FnMut` functions exists, if data needs to
/// be passed to the callback implement this over your struct.
pub trait BufferInputCallback: 'static {
    /// Callback that will be called when the buffer receives some input.
    ///
    /// # Arguments
    ///
    /// * `weechat` - A Weechat context.
    ///
    /// * `buffer` - The buffer that received the input
    ///
    /// * `input` - The input that was received.
    fn callback(&mut self, weechat: &Weechat, buffer: &Buffer, input: Cow<str>) -> Result<(), ()>;
}

impl<T: FnMut(&Weechat, &Buffer, Cow<str>) -> Result<(), ()> + 'static> BufferInputCallback for T {
    /// Callback that will be called if the user inputs something into the buffer
    /// input field.
    ///
    /// # Arguments
    ///
    /// * `weechat` - A Weechat context.
    ///
    /// * `buffer` - The buffer that the user inputed some text into.
    ///
    /// * `input` - The input that was posted by the user.
    fn callback(&mut self, weechat: &Weechat, buffer: &Buffer, input: Cow<str>) -> Result<(), ()> {
        self(weechat, buffer, input)
    }
}

/// Trait for the buffer close callback
///
/// A blanket implementation for pure `FnMut` functions exists, if data needs to
/// be passed to the callback implement this over your struct.
pub trait BufferCloseCallback {
    /// Callback that will be called before the buffer is closed.
    ///
    /// # Arguments
    ///
    /// * `weechat` - A Weechat context.
    ///
    /// * `buffer` - The buffer that will be closed.
    fn callback(&mut self, weechat: &Weechat, buffer: &Buffer) -> Result<(), ()>;
}

impl<T: FnMut(&Weechat, &Buffer) -> Result<(), ()> + 'static> BufferCloseCallback for T {
    fn callback(&mut self, weechat: &Weechat, buffer: &Buffer) -> Result<(), ()> {
        self(weechat, buffer)
    }
}

#[cfg(feature = "async")]
#[cfg_attr(feature = "docs", doc(cfg(r#async)))]
#[async_trait(?Send)]
/// Trait for the buffer input callback.
///
/// This is the async version of the callback.
///
/// A blanket implementation for pure `FnMut` functions exists, if data needs to
/// be passed to the callback implement this over your struct.
pub trait BufferInputCallbackAsync: 'static {
    /// Callback that will be called if the user inputs something into the buffer
    /// input field.
    ///
    /// # Arguments
    ///
    /// * `weechat` - A Weechat context.
    ///
    /// * `buffer` - The buffer that the user inputed some text into.
    ///
    /// * `input` - The input that was posted by the user.
    async fn callback(&mut self, buffer: BufferHandle, input: String);
}

#[cfg(feature = "async")]
#[async_trait(?Send)]
impl<T: FnMut(BufferHandle, String) -> LocalBoxFuture<'static, ()> + 'static>
    BufferInputCallbackAsync for T
{
    async fn callback(&mut self, buffer: BufferHandle, input: String) {
        self(buffer, input).await
    }
}

#[cfg(feature = "async")]
#[cfg_attr(feature = "docs", doc(cfg(r#async)))]
/// Builder for the creation of a buffer.
pub struct BufferBuilderAsync {
    pub(crate) name: String,
    pub(crate) input_callback: Option<Box<dyn BufferInputCallbackAsync>>,
    pub(crate) close_callback: Option<Box<dyn BufferCloseCallback>>,
}

/// Builder for the creation of a buffer.
pub struct BufferBuilder {
    pub(crate) name: String,
    pub(crate) input_callback: Option<Box<dyn BufferInputCallback>>,
    pub(crate) close_callback: Option<Box<dyn BufferCloseCallback>>,
}

#[cfg(feature = "async")]
impl BufferBuilderAsync {
    /// Create a new buffer builder that will create a buffer with an async
    /// input callback.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the new buffer. Needs to be unique across a
    /// plugin, otherwise the buffer creation will fail.
    ///
    /// Returns a Buffer if one has been created, otherwise an empty Error.
    ///
    /// # Panics
    ///
    /// Panics if the method is not called from the main Weechat thread.
    ///
    /// # Example
    /// ```no_run
    /// # use futures::future::{FutureExt, LocalBoxFuture};
    /// # use weechat::Weechat;
    /// # use weechat::buffer::{BufferHandle, Buffer, BufferBuilderAsync};
    /// fn input_cb(buffer: BufferHandle, input: String) -> LocalBoxFuture<'static, ()> {
    ///     async move {
    ///         let buffer = buffer.upgrade().unwrap();
    ///         buffer.print(&input);
    ///     }.boxed_local()
    /// }
    ///
    /// let buffer_handle = BufferBuilderAsync::new("test_buffer")
    ///     .input_callback(input_cb)
    ///     .close_callback(|weechat: &Weechat, buffer: &Buffer| {
    ///         Ok(())
    /// })
    ///     .build()
    ///     .expect("Can't create new buffer");
    ///
    /// let buffer = buffer_handle
    ///     .upgrade()
    ///     .expect("Can't upgrade newly created buffer");
    ///
    /// buffer.enable_nicklist();
    /// buffer.print("Hello world");
    /// ```
    pub fn new(name: &str) -> Self {
        BufferBuilderAsync {
            name: name.to_owned(),
            input_callback: None,
            close_callback: None,
        }
    }

    /// Set the buffer input callback.
    ///
    /// # Arguments
    ///
    /// * `callback` - An async function that will be called once a user inputs
    ///     data into the buffer input line.
    pub fn input_callback(mut self, callback: impl BufferInputCallbackAsync) -> Self {
        self.input_callback = Some(Box::new(callback));
        self
    }

    /// Set the close callback.
    ///
    /// # Arguments
    ///
    /// * `callback` - The callback that should be called before a buffer is
    ///     closed.
    pub fn close_callback(mut self, callback: impl BufferCloseCallback + 'static) -> Self {
        self.close_callback = Some(Box::new(callback));
        self
    }

    /// Build the configured buffer.
    pub fn build(self) -> Result<BufferHandle, ()> {
        Weechat::buffer_new_with_async(self)
    }
}

impl BufferBuilder {
    /// Create a new buffer builder that will create a buffer with an sync input
    /// callback.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the new buffer. Needs to be unique across a
    /// plugin, otherwise the buffer creation will fail.
    ///
    /// # Panics
    ///
    /// Panics if the method is not called from the main Weechat thread.
    ///
    /// Returns a Buffer if one has been created, otherwise an empty Error.
    ///
    /// # Example
    /// ```no_run
    /// # use std::borrow::Cow;
    /// # use weechat::Weechat;
    /// # use weechat::buffer::{Buffer, BufferHandle, BufferBuilder};
    /// fn input_cb(weechat: &Weechat, buffer: &Buffer, input: Cow<str>) -> Result<(), ()> {
    ///     buffer.print(&input);
    ///     Ok(())
    /// }
    ///
    /// let buffer_handle = BufferBuilder::new("test_buffer")
    ///     .input_callback(input_cb)
    ///     .close_callback(|weechat: &Weechat, buffer: &Buffer| {
    ///         Ok(())
    /// })
    ///     .build()
    ///     .expect("Can't create new buffer");
    ///
    /// let buffer = buffer_handle
    ///     .upgrade()
    ///     .expect("Can't upgrade newly created buffer");
    ///
    /// buffer.enable_nicklist();
    /// buffer.print("Hello world");
    /// ```
    pub fn new(name: &str) -> Self {
        BufferBuilder {
            name: name.to_owned(),
            input_callback: None,
            close_callback: None,
        }
    }

    /// Set the buffer input callback.
    ///
    /// # Arguments
    ///
    /// * `callback` - A function or a struct that implements the
    /// BufferCloseCallback trait.
    pub fn input_callback(mut self, callback: impl BufferInputCallback + 'static) -> Self {
        self.input_callback = Some(Box::new(callback));
        self
    }

    /// Set the close callback.
    ///
    /// # Arguments
    ///
    /// * `callback` - The callback that should be called before a buffer is
    pub fn close_callback(mut self, callback: impl BufferCloseCallback + 'static) -> Self {
        self.close_callback = Some(Box::new(callback));
        self
    }

    /// Build the configured buffer.
    pub fn build(self) -> Result<BufferHandle, ()> {
        Weechat::buffer_new(self)
    }
}

impl Weechat {
    /// Search a buffer by plugin and/or name.
    ///
    /// Returns a Buffer if one is found, otherwise None.
    ///
    /// # Arguments
    ///
    /// * `plugin_name` - name of a plugin, the following special value is
    ///     allowed: "==", the buffer name used is the buffers full name.
    ///
    /// * `buffer_name` - name of a buffer, if this is an empty string,
    ///     the current buffer is returned (buffer displayed by current
    ///     window); if the name starts with (?i), the search is case
    ///     insensitive.
    pub fn buffer_search(&self, plugin_name: &str, buffer_name: &str) -> Option<Buffer> {
        let buffer_search = self.get().buffer_search.unwrap();

        let plugin_name = LossyCString::new(plugin_name);
        let buffer_name = LossyCString::new(buffer_name);

        let buf_ptr = unsafe { buffer_search(plugin_name.as_ptr(), buffer_name.as_ptr()) };

        if buf_ptr.is_null() {
            None
        } else {
            Some(self.buffer_from_ptr(buf_ptr))
        }
    }

    pub(crate) fn buffer_from_ptr(&self, buffer_ptr: *mut t_gui_buffer) -> Buffer {
        Buffer {
            inner: InnerBuffers::BorrowedBuffer(InnerBuffer {
                weechat: self,
                ptr: buffer_ptr,
                closing: Rc::new(Cell::new(false)),
            }),
        }
    }

    /// Get the currently open buffer
    pub fn current_buffer(&self) -> Buffer {
        let buffer_search = self.get().buffer_search.unwrap();

        let buf_ptr = unsafe { buffer_search(ptr::null(), ptr::null()) };
        if buf_ptr.is_null() {
            panic!("No open buffer found");
        } else {
            self.buffer_from_ptr(buf_ptr)
        }
    }

    /// Get the main/core buffer.
    pub fn core_buffer(&self) -> Buffer {
        let buffer_search = self.get().buffer_search_main.unwrap();

        let buf_ptr = unsafe { buffer_search() };

        if buf_ptr.is_null() {
            panic!("No open buffer found");
        } else {
            self.buffer_from_ptr(buf_ptr)
        }
    }

    #[cfg(feature = "async")]
    #[cfg_attr(feature = "docs", doc(cfg(r#async)))]
    fn buffer_new_with_async(builder: BufferBuilderAsync) -> Result<BufferHandle, ()> {
        unsafe extern "C" fn c_input_cb(
            pointer: *const c_void,
            _data: *mut c_void,
            buffer: *mut t_gui_buffer,
            input_data: *const c_char,
        ) -> c_int {
            let input_data = CStr::from_ptr(input_data).to_string_lossy();

            let pointers: &mut BufferPointersAsync =
                { &mut *(pointer as *mut BufferPointersAsync) };

            let weechat = Weechat::from_ptr(pointers.weechat);
            let buffer = weechat.buffer_from_ptr(buffer);
            let buffer_cell = pointers
                .buffer_cell
                .as_ref()
                .expect("Buffer cell wasn't initialized properly")
                .clone();

            let buffer_handle = BufferHandle {
                buffer_name: Rc::new(buffer.full_name().to_string()),
                weechat: pointers.weechat,
                buffer_ptr: buffer_cell,
                closing: Rc::new(Cell::new(false)),
            };
            if let Some(cb) = pointers.input_cb.as_mut() {
                let future = cb.callback(buffer_handle, input_data.to_string());
                Weechat::spawn_buffer_cb(buffer.full_name().to_string(), future).detach();
            }

            WEECHAT_RC_OK
        }

        unsafe extern "C" fn c_close_cb(
            pointer: *const c_void,
            _data: *mut c_void,
            buffer: *mut t_gui_buffer,
        ) -> c_int {
            // We use from_raw() here so that the box gets deallocated at the
            // end of this scope.
            let pointers = Box::from_raw(pointer as *mut BufferPointersAsync);
            let weechat = Weechat::from_ptr(pointers.weechat);
            let buffer = weechat.buffer_from_ptr(buffer);
            buffer.mark_as_closing();

            let ret = if let Some(mut cb) = pointers.close_cb {
                cb.callback(&weechat, &buffer).is_ok()
            } else {
                true
            };

            // Invalidate the buffer pointer now.
            pointers
                .buffer_cell
                .as_ref()
                .expect("Buffer cell wasn't initialized properly")
                .replace(ptr::null_mut());

            if ret {
                WEECHAT_RC_OK
            } else {
                WEECHAT_RC_ERROR
            }
        }

        Weechat::check_thread();
        let weechat = unsafe { Weechat::weechat() };

        let c_input_cb: Option<WeechatInputCbT> = match builder.input_callback {
            Some(_) => Some(c_input_cb),
            None => None,
        };

        // We create a box and use leak to stop rust from freeing our data,
        // we are giving Weechat ownership over the data and will free it in
        // the buffer close callback.
        let buffer_pointers = Box::new(BufferPointersAsync {
            weechat: weechat.ptr,
            input_cb: builder.input_callback,
            close_cb: builder.close_callback,
            buffer_cell: None,
        });

        let buffer_pointers_ref = Box::leak(buffer_pointers);

        let buf_new = weechat.get().buffer_new.unwrap();
        let c_name = LossyCString::new(builder.name);

        let buf_ptr = unsafe {
            buf_new(
                weechat.ptr,
                c_name.as_ptr(),
                c_input_cb,
                buffer_pointers_ref as *const _ as *const c_void,
                ptr::null_mut(),
                Some(c_close_cb),
                buffer_pointers_ref as *const _ as *const c_void,
                ptr::null_mut(),
            )
        };

        if buf_ptr.is_null() {
            unsafe { Box::from_raw(buffer_pointers_ref) };
            return Err(());
        }

        let pointers: &mut BufferPointersAsync =
            unsafe { &mut *(buffer_pointers_ref as *mut BufferPointersAsync) };

        let buffer = weechat.buffer_from_ptr(buf_ptr);
        let buffer_cell = Rc::new(Cell::new(buf_ptr));

        pointers.buffer_cell = Some(buffer_cell.clone());

        Ok(BufferHandle {
            buffer_name: Rc::new(buffer.full_name().to_string()),
            weechat: weechat.ptr,
            buffer_ptr: buffer_cell,
            closing: Rc::new(Cell::new(false)),
        })
    }

    fn buffer_new(builder: BufferBuilder) -> Result<BufferHandle, ()> {
        unsafe extern "C" fn c_input_cb(
            pointer: *const c_void,
            _data: *mut c_void,
            buffer: *mut t_gui_buffer,
            input_data: *const c_char,
        ) -> c_int {
            let input_data = CStr::from_ptr(input_data).to_string_lossy();

            let pointers: &mut BufferPointers = { &mut *(pointer as *mut BufferPointers) };

            let weechat = Weechat::from_ptr(pointers.weechat);
            let buffer = weechat.buffer_from_ptr(buffer);

            let ret = if let Some(ref mut cb) = pointers.input_cb.as_mut() {
                cb.callback(&weechat, &buffer, input_data).is_ok()
            } else {
                true
            };

            if ret {
                WEECHAT_RC_OK
            } else {
                WEECHAT_RC_ERROR
            }
        }

        unsafe extern "C" fn c_close_cb(
            pointer: *const c_void,
            _data: *mut c_void,
            buffer: *mut t_gui_buffer,
        ) -> c_int {
            // We use from_raw() here so that the box gets freed at the end
            // of this scope.
            let pointers = Box::from_raw(pointer as *mut BufferPointers);
            let weechat = Weechat::from_ptr(pointers.weechat);
            let buffer = weechat.buffer_from_ptr(buffer);
            buffer.mark_as_closing();

            let ret = if let Some(mut cb) = pointers.close_cb {
                cb.callback(&weechat, &buffer).is_ok()
            } else {
                true
            };

            // Invalidate the buffer pointer now.
            pointers
                .buffer_cell
                .as_ref()
                .expect("Buffer cell wasn't initialized properly")
                .replace(ptr::null_mut());

            if ret {
                WEECHAT_RC_OK
            } else {
                WEECHAT_RC_ERROR
            }
        }

        Weechat::check_thread();
        let weechat = unsafe { Weechat::weechat() };

        let c_input_cb: Option<WeechatInputCbT> = match builder.input_callback {
            Some(_) => Some(c_input_cb),
            None => None,
        };

        // We create a box and use leak to stop rust from freeing our data,
        // we are giving weechat ownership over the data and will free it in
        // the buffer close callback.
        let buffer_pointers = Box::new(BufferPointers {
            weechat: weechat.ptr,
            input_cb: builder.input_callback,
            close_cb: builder.close_callback,
            buffer_cell: None,
        });
        let buffer_pointers_ref = Box::leak(buffer_pointers);

        let buf_new = weechat.get().buffer_new.unwrap();
        let c_name = LossyCString::new(builder.name);

        let buf_ptr = unsafe {
            buf_new(
                weechat.ptr,
                c_name.as_ptr(),
                c_input_cb,
                buffer_pointers_ref as *const _ as *const c_void,
                ptr::null_mut(),
                Some(c_close_cb),
                buffer_pointers_ref as *const _ as *const c_void,
                ptr::null_mut(),
            )
        };

        if buf_ptr.is_null() {
            unsafe { Box::from_raw(buffer_pointers_ref) };
            return Err(());
        }

        let pointers: &mut BufferPointers =
            unsafe { &mut *(buffer_pointers_ref as *mut BufferPointers) };

        let buffer = weechat.buffer_from_ptr(buf_ptr);
        let buffer_cell = Rc::new(Cell::new(buf_ptr));

        pointers.buffer_cell = Some(buffer_cell.clone());

        Ok(BufferHandle {
            buffer_name: Rc::new(buffer.full_name().to_string()),
            weechat: weechat.ptr,
            buffer_ptr: buffer_cell,
            closing: Rc::new(Cell::new(false)),
        })
    }
}

pub(crate) type WeechatInputCbT = unsafe extern "C" fn(
    pointer: *const c_void,
    data: *mut c_void,
    buffer: *mut t_gui_buffer,
    input_data: *const c_char,
) -> c_int;

impl Buffer<'_> {
    fn weechat(&self) -> &Weechat {
        match &self.inner {
            InnerBuffers::BorrowedBuffer(b) => &b.weechat,
            InnerBuffers::OwnedBuffer(b) => &b.weechat,
        }
    }

    pub(crate) fn ptr(&self) -> *mut t_gui_buffer {
        match &self.inner {
            InnerBuffers::BorrowedBuffer(b) => b.ptr,
            InnerBuffers::OwnedBuffer(b) => {
                let ptr = b.buffer_handle.buffer_ptr.get();

                if ptr.is_null() {
                    panic!("Buffer {} has been closed.", b.buffer_handle.buffer_name)
                } else {
                    ptr
                }
            }
        }
    }

    fn is_closing(&self) -> bool {
        self.inner.is_closing()
    }

    fn mark_as_closing(&self) {
        self.inner.mark_as_closing()
    }

    /// Display a message on the buffer.
    pub fn print(&self, message: &str) {
        let weechat = self.weechat();
        let printf_date_tags = weechat.get().printf_date_tags.unwrap();

        let fmt_str = LossyCString::new("%s");
        let c_message = LossyCString::new(message);

        unsafe {
            printf_date_tags(
                self.ptr(),
                0,
                ptr::null(),
                fmt_str.as_ptr(),
                c_message.as_ptr(),
            )
        }
    }

    /// Display a message on the buffer with attached date and tags
    ///
    /// # Arguments
    ///
    /// * `date` - A unix time-stamp representing the date of the message, 0
    ///     means now.
    ///
    /// * `tags` - A list of tags that will be applied to the printed line.
    ///
    /// * `message` - The message that will be displayed.
    pub fn print_date_tags(&self, date: i64, tags: &[&str], message: &str) {
        let weechat = self.weechat();
        let printf_date_tags = weechat.get().printf_date_tags.unwrap();

        let fmt_str = LossyCString::new("%s");
        let tags = tags.join(",");
        let tags = LossyCString::new(tags);
        let message = LossyCString::new(message);

        unsafe {
            printf_date_tags(
                self.ptr(),
                date,
                tags.as_ptr(),
                fmt_str.as_ptr(),
                message.as_ptr(),
            )
        }
    }

    /// Search for a nicklist group by name
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the nicklist that should be searched for.
    ///
    /// Returns a NickGroup if one is found, None otherwise.
    pub fn search_nicklist_group(&self, name: &str) -> Option<NickGroup> {
        let weechat = self.weechat();

        let nicklist_search_group = weechat.get().nicklist_search_group.unwrap();

        let name = LossyCString::new(name);

        let group = unsafe { nicklist_search_group(self.ptr(), ptr::null_mut(), name.as_ptr()) };

        if group.is_null() {
            None
        } else {
            Some(NickGroup {
                ptr: group,
                buf_ptr: self.ptr(),
                weechat_ptr: self.weechat().ptr,
                buffer: PhantomData,
            })
        }
    }

    /// Search for a nick in the whole nicklist.
    ///
    /// # Arguments
    ///
    /// * `nick` - The name of the nick that should be found.
    ///
    /// Returns a `Nick` if one is found, None otherwise.
    pub fn search_nick(&self, nick: &str) -> Option<Nick> {
        let weechat = self.weechat();
        let nick = Buffer::search_nick_helper(&weechat, self.ptr(), nick, None);

        if nick.is_null() {
            None
        } else {
            Some(Nick {
                ptr: nick,
                buf_ptr: self.ptr(),
                weechat_ptr: weechat.ptr,
                buffer: PhantomData,
            })
        }
    }

    fn search_nick_helper(
        weechat: &Weechat,
        buffer_ptr: *mut t_gui_buffer,
        nick: &str,
        group: Option<&NickGroup>,
    ) -> *mut t_gui_nick {
        let nicklist_search_nick = weechat.get().nicklist_search_nick.unwrap();

        let nick = LossyCString::new(nick);
        let group_ptr = group.map(|g| g.ptr).unwrap_or(ptr::null_mut());

        unsafe { nicklist_search_nick(buffer_ptr, group_ptr, nick.as_ptr()) }
    }

    /// Create and add a new nick to the buffer nicklist.
    ///
    /// This will add the nick to the root nick group.
    ///
    /// # Arguments
    ///
    /// * `nick_settings` - Nick arguments struct for the nick that should be
    ///     added.
    ///
    /// Returns the newly created nick if one is created successfully, an empty
    /// error otherwise.
    pub fn add_nick(&self, nick_settings: NickSettings) -> Result<Nick, ()> {
        let weechat = self.weechat();
        let nick_ptr = Buffer::add_nick_helper(&weechat, self.ptr(), nick_settings, None);

        if nick_ptr.is_null() {
            return Err(());
        }

        Ok(Nick {
            ptr: nick_ptr,
            buf_ptr: self.ptr(),
            weechat_ptr: self.weechat().ptr,
            buffer: PhantomData,
        })
    }

    /// Removes a group from the nicklist.
    ///
    /// # Arguments
    ///
    /// * `group_name` - The name of the group that should be removed.
    ///
    /// Returns `true` if a group was found and removed, `false` otherwise.
    pub fn remove_nicklist_group(&self, group_name: &str) -> bool {
        let weechat = self.weechat();

        let group = self.search_nicklist_group(group_name);

        match group {
            Some(group) => {
                let nicklist_remove_group = weechat.get().nicklist_remove_group.unwrap();

                unsafe {
                    nicklist_remove_group(self.ptr(), group.ptr);
                }
                true
            }
            None => false,
        }
    }

    /// Removes a nick from the nicklist.
    ///
    /// # Arguments
    ///
    /// * `nick` - The name of the nick that should be removed.
    ///
    /// Returns `true` if a nick was found and removed, `false` otherwise.
    pub fn remove_nick(&self, nick: &str) -> bool {
        let weechat = self.weechat();

        let nick = self.search_nick(nick);

        match nick {
            Some(nick) => {
                let nicklist_remove_nick = weechat.get().nicklist_remove_nick.unwrap();

                unsafe {
                    nicklist_remove_nick(self.ptr(), nick.ptr);
                }
                true
            }
            None => false,
        }
    }

    fn add_nick_helper(
        weechat: &Weechat,
        buffer_ptr: *mut t_gui_buffer,
        nick_settings: NickSettings,
        group: Option<&NickGroup>,
    ) -> *mut t_gui_nick {
        let c_nick = LossyCString::new(nick_settings.name);
        let color = LossyCString::new(nick_settings.color);
        let prefix = LossyCString::new(nick_settings.prefix);
        let prefix_color = LossyCString::new(nick_settings.prefix_color);

        let add_nick = weechat.get().nicklist_add_nick.unwrap();

        let group_ptr = match group {
            Some(g) => g.ptr,
            None => ptr::null_mut(),
        };

        unsafe {
            add_nick(
                buffer_ptr,
                group_ptr,
                c_nick.as_ptr(),
                color.as_ptr(),
                prefix.as_ptr(),
                prefix_color.as_ptr(),
                nick_settings.visible as i32,
            )
        }
    }

    /// Create and add a new nicklist group to the buffers nicklist.
    ///
    /// * `name` - Name of the new group.
    ///
    /// * `color` - Color of the new group.
    ///
    /// * `visible` - Should the group be visible in the nicklist.
    ///
    /// * `parent_group` - Parent group that the group should be added to.
    ///     If no group is provided the group is added to the root group.
    ///
    /// Returns the new nicklist group. The group is not removed if the object
    /// is dropped.
    pub fn add_nicklist_group(
        &self,
        name: &str,
        color: &str,
        visible: bool,
        parent_group: Option<&NickGroup>,
    ) -> Result<NickGroup, ()> {
        let weechat = self.weechat();
        let add_group = weechat.get().nicklist_add_group.unwrap();

        let c_name = LossyCString::new(name);
        let c_color = LossyCString::new(color);

        let group_ptr = match parent_group {
            Some(g) => g.ptr,
            None => ptr::null_mut(),
        };

        let group_ptr = unsafe {
            add_group(
                self.ptr(),
                group_ptr,
                c_name.as_ptr(),
                c_color.as_ptr(),
                visible as i32,
            )
        };

        if group_ptr.is_null() {
            return Err(());
        }

        Ok(NickGroup {
            ptr: group_ptr,
            buf_ptr: self.ptr(),
            weechat_ptr: self.weechat().ptr,
            buffer: PhantomData,
        })
    }

    fn set(&self, property: &str, value: &str) {
        let weechat = self.weechat();

        let buffer_set = weechat.get().buffer_set.unwrap();
        let option = LossyCString::new(property);
        let value = LossyCString::new(value);

        unsafe { buffer_set(self.ptr(), option.as_ptr(), value.as_ptr()) };
    }

    fn get_string(&self, property: &str) -> Option<Cow<str>> {
        let weechat = self.weechat();

        let buffer_get = weechat.get().buffer_get_string.unwrap();
        let property = LossyCString::new(property);

        unsafe {
            let value = buffer_get(self.ptr(), property.as_ptr());
            if value.is_null() {
                None
            } else {
                Some(CStr::from_ptr(value).to_string_lossy())
            }
        }
    }

    fn get_integer(&self, property: &str) -> i32 {
        let weechat = self.weechat();

        let buffer_get = weechat.get().buffer_get_integer.unwrap();
        let property = LossyCString::new(property);

        unsafe { buffer_get(self.ptr(), property.as_ptr()) }
    }

    /// Get the value of a buffer localvar
    ///
    /// # Arguments
    ///
    /// * `property` - The name of the property for which the value should be
    ///     fetched.
    pub fn get_localvar(&self, property: &str) -> Option<Cow<str>> {
        self.get_string(&format!("localvar_{}", property))
    }

    /// Set the value of a buffer localvar
    ///
    /// # Arguments
    ///
    /// * `property` - The property that should be set.
    ///
    /// * `value` - The value that the property should get.
    pub fn set_localvar(&self, property: &str, value: &str) {
        self.set(&format!("localvar_set_{}", property), value)
    }

    /// Get the full name of the buffer.
    pub fn full_name(&self) -> Cow<str> {
        self.get_string("full_name").unwrap()
    }

    /// Set the full name of the buffer
    ///
    /// # Arguments
    ///
    /// * `name` - The new full name that should be set.
    pub fn set_full_name(&self, name: &str) {
        self.set("full_name", name);
    }

    /// Get the name of the buffer.
    pub fn name(&self) -> Cow<str> {
        self.get_string("name").unwrap()
    }

    /// Set the name of the buffer.
    ///
    /// # Arguments
    ///
    /// * `name` - The new name that should be set.
    pub fn set_name(&self, name: &str) {
        self.set("name", name);
    }

    /// Get the short_name of the buffer.
    pub fn short_name(&self) -> Cow<str> {
        self.get_string("short_name").unwrap()
    }

    /// Set the short_name of the buffer.
    ///
    /// # Arguments
    ///
    /// * `name` - The new short name that should be set.
    pub fn set_short_name(&self, name: &str) {
        self.set("short_name", name);
    }

    /// Get the plugin name of the plugin that owns this buffer.
    pub fn plugin_name(&self) -> Cow<str> {
        self.get_string("plugin").unwrap()
    }

    /// Hide time for all lines in the buffer.
    pub fn disable_time_for_each_line(&self) {
        self.set("time_for_each_line", "0");
    }

    /// Disable the nicklist for this buffer.
    pub fn disable_nicklist(&self) {
        self.set("nicklist", "0")
    }

    /// Enable displaying of groups in the nicklist.
    pub fn enable_nicklist_groups(&self) {
        self.set("nicklist_display_groups", "1")
    }

    /// Disable displaying of groups in the nicklist.
    pub fn disable_nicklist_groups(&self) {
        self.set("nicklist_display_groups", "0")
    }

    /// Enable the nicklist for this buffer.
    pub fn enable_nicklist(&self) {
        self.set("nicklist", "1")
    }

    /// Get the title of the buffer
    pub fn title(&self) {
        self.get_string("title");
    }

    /// Set the title of the buffer.
    ///
    /// # Arguments
    ///
    /// * `title` - The new title that will be set.
    pub fn set_title(&self, title: &str) {
        self.set("title", title);
    }

    /// Disable logging for this buffer.
    pub fn disable_log(&self) {
        self.set("localvar_set_no_log", "1");
    }

    /// Clear buffer contents
    pub fn clear(&self) {
        let weechat = self.weechat();

        let buffer_clear = weechat.get().buffer_clear.unwrap();
        unsafe { buffer_clear(self.ptr()) }
    }

    /// Close the buffer.
    ///
    /// Note that this will only queue up the buffer to be closed. The close
    /// callback will first be run, only then will the buffer be closed.
    ///
    /// Multiple calls to this method will result in a nop.
    pub fn close(&self) {
        if !self.is_closing() {
            let weechat = self.weechat();

            let buffer_close = weechat.get().buffer_close.unwrap();
            unsafe { buffer_close(self.ptr()) };
            self.mark_as_closing();
        }
    }

    /// Get the contents of the input
    pub fn input(&self) -> Cow<str> {
        self.get_string("input").unwrap()
    }

    /// Set the content of the buffer input.
    pub fn set_input(&self, input: &str) {
        self.set("input", input)
    }

    /// Get the position of the cursor in the buffer input.
    pub fn input_position(&self) -> i32 {
        self.get_integer("input_pos")
    }

    /// Set the position of the input buffer.
    ///
    /// # Arguments
    ///
    /// * `position` - The new position of the input.
    pub fn set_input_position(&self, position: i32) {
        self.set("input_pos", &position.to_string())
    }

    /// Enable multiline support.
    ///
    /// If enabled multiple lines separated by a newline are sent as a single
    /// string to the input callback. If disabled each line will be sent to the
    /// input callback separately.
    pub fn enable_multiline(&self) {
        self.set("input_multiline", &1.to_string());
    }

    /// Disable multiline support.
    ///
    /// If enabled multiple lines separated by a newline are sent as a single
    /// string to the input callback. If disabled each line will be sent to the
    pub fn disable_multiline(&self) {
        self.set("input_multiline", &0.to_string());
    }

    /// Is multiline support enabled for this buffer.
    pub fn is_multiline_enabled(&self) -> bool {
        self.get_integer("input_multiline") == 1
    }

    /// Get the number of the buffer.
    pub fn number(&self) -> i32 {
        self.get_integer("number")
    }

    /// Switch to the buffer
    pub fn switch_to(&self) {
        self.set("display", "1");
    }

    /// Get the main/core buffer
    pub fn core_buffer(&self) -> Buffer {
        self.weechat().core_buffer()
    }

    /// Merge two buffers.
    pub fn merge(&self, target_buffer: &Buffer) {
        let weechat = self.weechat();

        if target_buffer != self {
            let merge = weechat.get().buffer_merge.unwrap();
            unsafe { merge(self.ptr(), target_buffer.ptr()) };
        }
    }

    /// Unmerge the buffer if it's merged with other buffers, the buffer will be
    /// moved to the current buffer number + 1.
    pub fn unmerge(&self) {
        self.unmerge_helper(None);
    }

    /// Unmerge the buffer if it's merged with other buffers, the buffer will be
    /// moved to target number.
    pub fn unmerge_to(&self, target_number: u16) {
        self.unmerge_helper(Some(target_number));
    }

    fn unmerge_helper(&self, target_number: Option<u16>) {
        let weechat = self.weechat();

        let unmerge = weechat.get().buffer_unmerge.unwrap();
        unsafe { unmerge(self.ptr(), target_number.map(|n| n.into()).unwrap_or(-1)) };
    }

    /// Run the given command in the buffer.
    ///
    /// # Arguments
    ///
    /// * `command` - The command that should run.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use weechat::Weechat;
    /// # use weechat::buffer::BufferBuilder;
    /// # let buffer_handle = BufferBuilder::new("test")
    /// #    .build()
    /// #    .unwrap();
    /// # let buffer = buffer_handle.upgrade().unwrap();
    ///
    /// // Switch to the core buffer using a command.
    /// buffer.run_command("/buffer core");
    /// ```
    pub fn run_command(&self, command: &str) -> Result<(), ()> {
        let command = LossyCString::new(command);
        let weechat = self.weechat();
        let run_command = weechat.get().command.unwrap();

        let ret = unsafe { run_command(weechat.ptr, self.ptr(), command.as_ptr()) };

        match ret {
            WEECHAT_RC_OK => Ok(()),
            WEECHAT_RC_ERROR => Err(()),
            _ => unreachable!(),
        }
    }

    fn hdata_pointer(&self) -> *mut t_hdata {
        let weechat = self.weechat();

        unsafe { weechat.hdata_get("buffer") }
    }

    fn own_lines(&self) -> *mut c_void {
        let weechat = self.weechat();

        let hdata = self.hdata_pointer();

        unsafe { weechat.hdata_pointer(hdata, self.ptr() as *mut c_void, "own_lines") }
    }

    /// Get the number of lines that the buffer has printed out.
    pub fn num_lines(&self) -> i32 {
        let weechat = self.weechat();
        let own_lines = self.own_lines();

        unsafe {
            let lines = weechat.hdata_get("lines");
            weechat.hdata_integer(lines, own_lines, "lines_count")
        }
    }

    /// Get the lines of the buffer.
    ///
    /// This returns an iterator over all the buffer lines, the iterator can be
    /// traversed forwards (from the first line of the buffer, to the last) as
    /// well as backwards (from the last line of the buffer to the first).
    ///
    /// # Example
    /// ```no_run
    /// # use weechat::Weechat;
    /// # use weechat::buffer::BufferBuilder;
    /// # let buffer_handle = BufferBuilder::new("test")
    /// #    .build()
    /// #    .unwrap();
    /// # let buffer = buffer_handle.upgrade().unwrap();
    ///
    /// let lines = buffer.lines();
    ///
    /// for line in lines {
    ///     Weechat::print(&format!("{:?}", line.tags()));
    /// }
    /// ```
    pub fn lines(&self) -> BufferLines {
        let weechat = self.weechat();

        let own_lines = self.own_lines();

        let (first_line, last_line) = unsafe {
            let lines = weechat.hdata_get("lines");

            (
                weechat.hdata_pointer(lines, own_lines, "first_line"),
                weechat.hdata_pointer(lines, own_lines, "last_line"),
            )
        };

        BufferLines {
            weechat_ptr: self.weechat().ptr,
            first_line,
            last_line,
            buffer: PhantomData,
            done: false,
        }
    }

    /// Get the window object that is currently displaying this buffer.
    ///
    /// Is `None` if no window is displaying this buffer.
    pub fn window(&self) -> Option<Window> {
        let weechat = self.weechat();
        let get_window = weechat.get().window_search_with_buffer.unwrap();

        let ptr = unsafe { get_window(self.ptr()) };

        if ptr.is_null() {
            None
        } else {
            Some(Window {
                weechat: weechat.ptr,
                ptr,
                phantom: PhantomData,
            })
        }
    }

    /// Hide the buffer from the buflist.
    pub fn hide(&self) {
        self.set("hidden", "1");
    }

    /// Unhide the buffer from the buflist.
    pub fn unhide(&self) {
        self.set("hidden", "0");
    }
}
