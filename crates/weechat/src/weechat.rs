//! Main weechat module

use backtrace::Backtrace;

use weechat_sys::t_weechat_plugin;

use crate::LossyCString;
use libc::{c_char, c_int};
use std::{
    ffi::{CStr, CString},
    panic::PanicInfo,
    path::PathBuf,
    ptr, vec,
};

#[cfg(feature = "async")]
use crate::executor::WeechatExecutor;
#[cfg(feature = "async")]
pub use async_task::Task;
#[cfg(feature = "async")]
use std::future::Future;

/// An iterator over the arguments of a Weechat command, yielding a String value
/// for each argument.
pub struct Args {
    iter: vec::IntoIter<String>,
}

/// A Weechat prefix, can be prepended to a message to notify the message
/// category.
pub enum Prefix {
    /// Prefix for an error message.
    Error,
    /// Prefix for a networking related message.
    Network,
    /// Prefix for a `/me` action type of message.
    Action,
    /// Prefix for a message notifying that an user has joined the chat.
    Join,
    /// Prefix for a message notifying that an user has left the chat.
    Quit,
}

impl Prefix {
    fn as_str(&self) -> &str {
        match self {
            Prefix::Error => "error",
            Prefix::Network => "network",
            Prefix::Action => "action",
            Prefix::Join => "join",
            Prefix::Quit => "quit",
        }
    }
}

impl Args {
    /// Create an Args object from the underlying weechat C types.
    /// Expects the strings in argv to be valid utf8, if not invalid UTF-8
    /// sequences are replaced with the replacement character.
    ///
    /// # Safety
    ///
    /// This should never be called by the user, this is called internally but
    /// needs to be public because it's used in the macro expansion of the
    /// plugin init method.
    #[doc(hidden)]
    pub fn new(argc: c_int, argv: *mut *mut c_char) -> Args {
        let argc = argc as isize;
        let args: Vec<String> = (0..argc)
            .map(|i| {
                let cstr = unsafe { CStr::from_ptr(*argv.offset(i) as *const libc::c_char) };

                String::from_utf8_lossy(&cstr.to_bytes().to_vec()).to_string()
            })
            .collect();
        Args {
            iter: args.into_iter(),
        }
    }
}

impl std::fmt::Debug for Args {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.iter.clone()).finish()
    }
}

impl Iterator for Args {
    type Item = String;

    fn next(&mut self) -> Option<String> {
        self.iter.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl ExactSizeIterator for Args {
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl DoubleEndedIterator for Args {
    fn next_back(&mut self) -> Option<String> {
        self.iter.next_back()
    }
}

static mut WEECHAT: Option<Weechat> = None;
static mut WEECHAT_THREAD_ID: Option<std::thread::ThreadId> = None;

/// Main Weechat struct that encapsulates common weechat API functions.
/// It has a similar API as the weechat script API.
pub struct Weechat {
    pub(crate) ptr: *mut t_weechat_plugin,
}

impl Weechat {
    /// Create a Weechat object from a C t_weechat_plugin pointer.
    ///
    /// # Arguments
    ///
    /// * `ptr` - C pointer of the weechat plugin.
    ///
    /// # Safety
    ///
    /// This should never be called by the user. This is called internally.
    #[doc(hidden)]
    pub unsafe fn init_from_ptr(ptr: *mut t_weechat_plugin) -> Weechat {
        assert!(!ptr.is_null());

        WEECHAT = Some(Weechat { ptr });
        WEECHAT_THREAD_ID = Some(std::thread::current().id());

        std::panic::set_hook(Box::new(Weechat::panic_hook));

        #[cfg(feature = "async")]
        WeechatExecutor::start();
        Weechat { ptr }
    }

    fn panic_hook(info: &PanicInfo) {
        let current_thread = std::thread::current();
        let weechat_thread = Weechat::thread_id();

        let current_thread_id = current_thread.id();
        let thread_name = current_thread.name().unwrap_or("Unnamed");

        let backtrace = std::env::var("RUST_BACKTRACE")
            .map(|v| v == "1")
            .unwrap_or(false);

        if current_thread_id == weechat_thread {
            if backtrace {
                let bt = Backtrace::new();

                Weechat::print(&format!(
                    "{}Panic in the main Weechat thread: {}\n{:?}",
                    Weechat::prefix(Prefix::Error),
                    info,
                    bt,
                ));
            } else {
                Weechat::print(&format!(
                    "{}Panic in the main Weechat thread: {}",
                    Weechat::prefix(Prefix::Error),
                    info,
                ));
            }
        } else {
            #[cfg(feature = "async")]
            {
                let bt = if backtrace {
                    Some(Backtrace::new())
                } else {
                    None
                };

                if current_thread_id != weechat_thread {
                    Weechat::spawn_from_thread(Weechat::thread_panic(
                        thread_name.to_string(),
                        info.to_string(),
                        bt,
                    ))
                }
            }
            #[cfg(not(feature = "async"))]
            {
                println!("thread '{}' panicked: {}", thread_name, info);
            }
        }
    }

    #[cfg(feature = "async")]
    async fn thread_panic(thread_name: String, message: String, backtrace: Option<Backtrace>) {
        if let Some(backtrace) = backtrace {
            Weechat::print(&format!(
                "{}Thread '{}{}{}' {}\n{:?}",
                Weechat::prefix(Prefix::Error),
                Weechat::color("red"),
                thread_name,
                Weechat::color("reset"),
                message,
                backtrace,
            ));
        } else {
            Weechat::print(&format!(
                "{}Thread '{}{}{}' {}.",
                Weechat::prefix(Prefix::Error),
                Weechat::color("red"),
                thread_name,
                Weechat::color("reset"),
                message
            ));
        }
    }

    /// Free internal plugin data.
    /// # Safety
    ///
    /// This should never be called by the user. This is called internally.
    #[doc(hidden)]
    pub unsafe fn free() {
        #[cfg(feature = "async")]
        WeechatExecutor::free();
    }

    pub(crate) fn from_ptr(ptr: *mut t_weechat_plugin) -> Weechat {
        assert!(!ptr.is_null());
        Weechat { ptr }
    }

    /// Get the Weechat plugin.
    ///
    /// # Safety
    ///
    /// It is generally safe to call this method, the plugin pointer is valid
    /// for the durration of the plugin lifetime. The problem is that many
    /// Weechat objects need to have a lifetime bound to a Weechat context
    /// object that is only valid for the duration of a callback.
    ///
    /// Since this one will have a static lifetime, objects that are fetched
    /// from this object may have a longer lifetime than they should.
    pub unsafe fn weechat() -> &'static mut Weechat {
        match WEECHAT {
            Some(ref mut w) => w,
            None => panic!("Plugin wasn't initialized correctly"),
        }
    }

    #[inline]
    pub(crate) fn get(&self) -> &t_weechat_plugin {
        unsafe { &*self.ptr }
    }

    /// Write a message in WeeChat log file (weechat.log).
    ///
    /// # Panics
    ///
    /// Panics if the method is not called from the main Weechat thread.
    pub fn log(msg: &str) {
        Weechat::check_thread();
        let weechat = unsafe { Weechat::weechat() };
        let log_printf = weechat.get().log_printf.unwrap();

        let fmt = LossyCString::new("%s");
        let msg = LossyCString::new(msg);

        unsafe {
            log_printf(fmt.as_ptr(), msg.as_ptr());
        }
    }

    /// Display a message on the core weechat buffer.
    ///
    /// # Panics
    ///
    /// Panics if the method is not called from the main Weechat thread.
    pub fn print(msg: &str) {
        Weechat::check_thread();
        let weechat = unsafe { Weechat::weechat() };

        let printf_date_tags = weechat.get().printf_date_tags.unwrap();

        let fmt = LossyCString::new("%s");
        let msg = LossyCString::new(msg);

        unsafe {
            printf_date_tags(ptr::null_mut(), 0, ptr::null(), fmt.as_ptr(), msg.as_ptr());
        }
    }

    fn thread_id() -> std::thread::ThreadId {
        *unsafe {
            WEECHAT_THREAD_ID.as_ref().expect(
                "Weechat main thread ID wasn't found, plugin \
                 wasn't correctly initialized",
            )
        }
    }

    pub(crate) fn check_thread() {
        let weechat_thread_id = unsafe {
            WEECHAT_THREAD_ID.as_ref().expect(
                "Weechat main thread ID wasn't found, plugin \
                 wasn't correctly initialized",
            )
        };

        if std::thread::current().id() != *weechat_thread_id {
            panic!(
                "Weechat methods can be only called from the main Weechat \
                 thread."
            )
        }
    }

    /// Return a string color code for display.
    ///
    /// # Arguments
    ///
    /// `color_name` - name of the color
    ///
    /// # Panics
    ///
    /// Panics if the method is not called from the main Weechat thread.
    pub fn color(color_name: &str) -> &str {
        Weechat::check_thread();
        let weechat = unsafe { Weechat::weechat() };
        let weechat_color = weechat.get().color.unwrap();

        let color_name = LossyCString::new(color_name);
        unsafe {
            let color = weechat_color(color_name.as_ptr());
            CStr::from_ptr(color)
                .to_str()
                .expect("Weechat returned a non UTF-8 string")
        }
    }

    /// Return a string color pair for display.
    ///
    /// # Arguments
    ///
    /// `foreground_color` - Name of the color that should be used for the
    ///     foreground.
    ///
    /// `background_color` - Name of the color that should be used for the
    ///     background.
    ///
    /// # Panics
    ///
    /// Panics if the method is not called from the main Weechat thread.
    pub fn color_pair(foreground_color: &str, background_color: &str) -> String {
        Weechat::color(&format!("{},{}", foreground_color, background_color)).to_string()
    }

    /// Retrieve a prefix value
    ///
    /// # Arguments:
    ///
    /// `prefix` - The name of the prefix.
    ///
    /// Valid prefixes are:
    /// * error
    /// * network
    /// * action
    /// * join
    /// * quit
    ///
    /// An empty string will be returned if the prefix is not found
    ///
    /// # Panics
    ///
    /// Panics if the method is not called from the main Weechat thread.
    pub fn prefix(prefix: Prefix) -> String {
        Weechat::check_thread();
        let weechat = unsafe { Weechat::weechat() };

        let prefix_fn = weechat.get().prefix.unwrap();
        let prefix = LossyCString::new(prefix.as_str());

        unsafe {
            CStr::from_ptr(prefix_fn(prefix.as_ptr()))
                .to_str()
                .expect("Weechat returned a non UTF-8 string")
                .to_string()
        }
    }

    /// Get some info from Weechat or a plugin.
    ///
    /// # Arguments
    ///
    /// * `name` - name the info
    ///
    /// * `arguments` - arguments for the info
    pub fn info_get(name: &str, arguments: &str) -> Option<String> {
        Weechat::check_thread();
        let weechat = unsafe { Weechat::weechat() };

        let info_get = weechat.get().info_get.unwrap();

        let info_name = LossyCString::new(name);
        let arguments = LossyCString::new(arguments);

        unsafe {
            let info = info_get(weechat.ptr, info_name.as_ptr(), arguments.as_ptr());
            if info.is_null() {
                None
            } else {
                Some(CStr::from_ptr(info).to_string_lossy().to_string())
            }
        }
    }

    /// Remove WeeChat colors from a string.
    ///
    /// # Arguments
    ///
    /// * `string` - The string that should be stripped from Weechat colors.
    ///
    /// # Panics
    ///
    /// Panics if the method is not called from the main Weechat thread.
    pub fn remove_color(string: &str) -> String {
        Weechat::check_thread();
        let weechat = unsafe { Weechat::weechat() };

        let string = LossyCString::new(string);

        let remove_color = weechat.get().string_remove_color.unwrap();

        let string = unsafe {
            let ptr = remove_color(string.as_ptr(), ptr::null());
            CString::from_raw(ptr)
        };

        string.to_string_lossy().to_string()
    }

    /// Evaluate a Weechat expression and return the result.
    ///
    /// # Arguments
    ///
    /// * `expression` - The expression that should be evaluated.
    ///
    /// # Panics
    ///
    /// Panics if the method is not called from the main Weechat thread.
    //
    // TODO: Add hashtable options
    // TODO: This needs better docs and examples.
    pub fn eval_string_expression(expression: &str) -> Result<String, ()> {
        Weechat::check_thread();
        let weechat = unsafe { Weechat::weechat() };

        let string_eval_expression = weechat.get().string_eval_expression.unwrap();

        let expr = LossyCString::new(expression);

        unsafe {
            let result = string_eval_expression(
                expr.as_ptr(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
            );

            if result.is_null() {
                Err(())
            } else {
                Ok(CStr::from_ptr(result).to_string_lossy().to_string())
            }
        }
    }

    /// Get the Weechat homedir.
    pub fn home_dir() -> PathBuf {
        Weechat::check_thread();
        let weechat = unsafe { Weechat::weechat() };

        let eval_path_home = weechat.get().string_eval_path_home.unwrap();

        let path = LossyCString::new("%h");

        let path = unsafe {
            let result = eval_path_home(
                path.as_ptr(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
            );

            if result.is_null() {
                panic!("Returned null while evaluating the Weechat home dir");
            } else {
                CStr::from_ptr(result).to_string_lossy().to_string()
            }
        };

        PathBuf::from(path)
    }

    /// Replace a leading `~` with the home directory.
    ///
    /// If the string does not start with `~`, the same string is returned.
    pub fn expand_home(string: &str) -> String {
        Weechat::check_thread();

        let weechat = unsafe { Weechat::weechat() };
        let expand = weechat.get().string_expand_home.unwrap();
        let string = LossyCString::new(string);

        let string = unsafe {
            let result = expand(string.as_ptr());

            if result.is_null() {
                panic!("Returned null while expanding the home dir");
            } else {
                CStr::from_ptr(result).to_string_lossy().to_string()
            }
        };

        string
    }

    /// Execute a modifier.
    ///
    /// A modifier takes a string and modifies it in some way, Weechat has a
    /// list of defined modifiers. For example to parse a string with some color
    /// format (ansi, irc...) and to convert it to another format.
    ///
    /// Returns the modified string or an empty error if the string couldn't be
    /// modified.
    ///
    /// # Arguments
    ///
    /// * `modifier` - The name of a modifier. The list of modifiers can be found in
    ///     the official
    /// [Weechat documentation](https://weechat.org/files/doc/stable/weechat_plugin_api.en.html#_hook_modifier_exec).
    ///
    /// * `modifier_data` - Data that will be passed to the modifier, this
    /// depends on the modifier that was chosen, consult the list of modifiers
    /// in the Weechat documentation.
    ///
    /// * `input_string` - The string that should be modified.
    ///
    /// # Panics
    ///
    /// Panics if the method is not called from the main Weechat thread.
    pub fn execute_modifier(
        modifier: &str,
        modifier_data: &str,
        input_string: &str,
    ) -> Result<String, ()> {
        Weechat::check_thread();
        let weechat = unsafe { Weechat::weechat() };

        let exec = weechat.get().hook_modifier_exec.unwrap();

        let modifier = LossyCString::new(modifier);
        let modifier_data = LossyCString::new(modifier_data);
        let input_string = LossyCString::new(input_string);

        unsafe {
            let result = exec(
                weechat.ptr,
                modifier.as_ptr(),
                modifier_data.as_ptr(),
                input_string.as_ptr(),
            );

            if result.is_null() {
                Err(())
            } else {
                Ok(CStr::from_ptr(result).to_string_lossy().to_string())
            }
        }
    }

    /// Update the content of a bar item, by calling its build callback.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the bar item that should be updated.
    pub fn bar_item_update(name: &str) {
        Weechat::check_thread();
        let weechat = unsafe { Weechat::weechat() };

        let bar_item_update = weechat.get().bar_item_update.unwrap();

        let name = LossyCString::new(name);

        unsafe { bar_item_update(name.as_ptr()) }
    }

    /// Spawn a new `Future` on the main Weechat thread.
    ///
    /// # Panics
    ///
    /// Panics if the method is not called from the main Weechat thread or if
    /// the method is called in a buffer close callback when Weechat is shutting
    /// down.
    ///
    /// The buffer close callback gets called after the plugin is dropped and
    /// the executor will be stopped at that point.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use weechat::Weechat;
    /// use async_std::channel::{bounded as channel, Receiver};
    /// use futures::executor::block_on;
    ///
    /// pub async fn task(receiver: Receiver<String>) {
    ///     loop {
    ///         match receiver.recv().await {
    ///             Ok(m) => {
    ///                 Weechat::print(&format!("Received message: {}", m));
    ///             },
    ///             Err(e) => {
    ///                 Weechat::print(
    ///                     &format!("Error receiving on channel {:?}", e)
    ///                 );
    ///                 return;
    ///             }
    ///         }
    ///     }
    /// }
    ///
    /// let (tx, rx) = channel(1000);
    ///
    /// Weechat::spawn(task(rx));
    /// block_on(tx.send("Hello world".to_string()));
    /// ```
    #[cfg(feature = "async")]
    #[cfg_attr(feature = "docs", doc(cfg(r#async)))]
    pub fn spawn<F>(future: F) -> Task<F::Output>
    where
        F: Future + 'static,
        F::Output: 'static,
    {
        Weechat::check_thread();
        WeechatExecutor::spawn(future).expect("Executor isn't running anymore")
    }

    /// Spawn a new `Future` on the main Weechat thread, checking if the
    /// executor is running.
    ///
    /// # Panics
    ///
    /// Panics if the method is not called from the main Weechat thread.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use weechat::Weechat;
    /// use async_std::channel::{bounded as channel, Receiver};
    /// use futures::executor::block_on;
    ///
    /// pub async fn task(receiver: Receiver<String>) {
    ///     loop {
    ///         match receiver.recv().await {
    ///             Ok(m) => {
    ///                 Weechat::print(&format!("Received message: {}", m));
    ///             },
    ///             Err(e) => {
    ///                 Weechat::print(
    ///                     &format!("Error receiving on channel {:?}", e)
    ///                 );
    ///                 return;
    ///             }
    ///         }
    ///     }
    /// }
    ///
    /// let (tx, rx) = channel(1000);
    ///
    /// if let Some(task) = Weechat::spawn_checked(task(rx)) {
    ///     block_on(tx.send("Hello world".to_string()));
    /// }
    /// ```
    #[cfg(feature = "async")]
    #[cfg_attr(feature = "docs", doc(cfg(r#async)))]
    pub fn spawn_checked<F>(future: F) -> Option<Task<F::Output>>
    where
        F: Future + 'static,
        F::Output: 'static,
    {
        Weechat::check_thread();
        WeechatExecutor::spawn(future)
    }

    /// Spawn a new `Future` on the main Weechat thread.
    ///
    /// This can be called from any thread and will execute the future on the
    /// main Weechat thread.
    #[cfg(feature = "async")]
    #[cfg_attr(feature = "docs", doc(cfg(r#async)))]
    pub fn spawn_from_thread<F>(future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        WeechatExecutor::spawn_from_non_main(future)
    }

    #[cfg(feature = "async")]
    pub(crate) fn spawn_buffer_cb<F>(buffer_name: String, future: F) -> Task<F::Output>
    where
        F: Future + 'static,
        F::Output: 'static,
    {
        WeechatExecutor::spawn_buffer_cb(buffer_name, future)
    }
}
