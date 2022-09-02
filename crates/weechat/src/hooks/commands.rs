use libc::{c_char, c_int};
use std::{borrow::Cow, ffi::CStr, os::raw::c_void, ptr};

use weechat_sys::{t_gui_buffer, t_weechat_plugin, WEECHAT_RC_OK};

use crate::{buffer::Buffer, Args, LossyCString, ReturnCode, Weechat};

use super::Hook;

/// Hook for a weechat command, the command is removed when the object is
/// dropped.
pub struct Command {
    _hook: Hook,
    _hook_data: Box<CommandHookData>,
}

/// Trait for the command callback
///
/// A blanket implementation for pure `FnMut` functions exists, if data needs to
/// be passed to the callback implement this over your struct.
pub trait CommandCallback {
    /// Callback that will be called when the command is executed.
    ///
    /// # Arguments
    ///
    /// * `weechat` - A Weechat context.
    ///
    /// * `buffer` - The buffer that received the command.
    ///
    /// * `arguments` - The arguments that were passed to the command, this will
    ///     include the command as the first argument.
    fn callback(&mut self, weechat: &Weechat, buffer: &Buffer, arguments: Args);
}

impl<T: FnMut(&Weechat, &Buffer, Args) + 'static> CommandCallback for T {
    fn callback(&mut self, weechat: &Weechat, buffer: &Buffer, arguments: Args) {
        self(weechat, buffer, arguments)
    }
}

#[derive(Default)]
/// Description for a new Weechat command that should be created.
///
/// The fields of this struct accept the same string formats that are described
/// in the Weechat API documentation.
pub struct CommandSettings {
    /// Name of the command.
    name: String,
    /// Description for the command (displayed with `/help command`)
    description: String,
    /// Arguments for the command (displayed with `/help command`)
    arguments: Vec<String>,
    /// Description for the command arguments (displayed with `/help command`)
    argument_descriptoin: String,
    /// Completion template for the command.
    completion: Vec<String>,
}

impl CommandSettings {
    /// Create new command settings.
    ///
    /// This describes how a command will be created.
    ///
    /// #Arguments
    ///
    /// * `name` - The name that the section should get.
    pub fn new<P: Into<String>>(name: P) -> Self {
        CommandSettings {
            name: name.into(),
            ..Default::default()
        }
    }

    /// Set the description of the command.
    ///
    /// # Arguments
    ///
    /// * `description` - The description of the command.
    pub fn description<D: Into<String>>(mut self, descritpion: D) -> Self {
        self.description = descritpion.into();
        self
    }

    /// Add an argument to the command.
    ///
    /// Multiple arguments can be added to a command. See the `Command`
    /// documentation for an example of this.
    ///
    /// # Arguments
    ///
    /// * `argument` - The argument that should be added.
    pub fn add_argument<T: Into<String>>(mut self, argument: T) -> Self {
        self.arguments.push(argument.into());
        self
    }

    /// Set the description of the arguments.
    ///
    /// # Arguments
    ///
    /// * `description` - The argument description that should be set for the
    ///     command.
    pub fn arguments_description<T: Into<String>>(mut self, descritpion: T) -> Self {
        self.argument_descriptoin = descritpion.into();
        self
    }

    /// Add a completion definition to the command.
    ///
    /// Multiple arguments can be added to a command. See the `Command`
    /// documentation for an example of this.
    ///
    /// # Arguments
    ///
    /// * `completion` - The completion that should be added to the command.
    pub fn add_completion<T: Into<String>>(mut self, completion: T) -> Self {
        self.completion.push(completion.into());
        self
    }
}

struct CommandHookData {
    callback: Box<dyn CommandCallback>,
    weechat_ptr: *mut t_weechat_plugin,
}

/// Hook for a weechat command, the hook is removed when the object is dropped.
pub struct CommandRun {
    _hook: Hook,
    _hook_data: Box<CommandRunHookData>,
}

/// Trait for the command-run callback
///
/// A blanket implementation for pure `FnMut` functions exists, if data needs to
/// be passed to the callback implement this over your struct.
pub trait CommandRunCallback {
    /// Callback that will be called when the command is ran.
    ///
    /// Should return a code signaling if further command callbacks should be
    /// ran or if the command should be "eaten" by this callback.
    ///
    /// # Arguments
    ///
    /// * `weechat` - A Weechat context.
    ///
    /// * `buffer` - The buffer that received the command.
    ///
    /// * `command` - The full command that was executed, including its
    ///     arguments.
    fn callback(&mut self, weechat: &Weechat, buffer: &Buffer, command: Cow<str>) -> ReturnCode;
}

impl<T: FnMut(&Weechat, &Buffer, Cow<str>) -> ReturnCode + 'static> CommandRunCallback for T {
    fn callback(&mut self, weechat: &Weechat, buffer: &Buffer, command: Cow<str>) -> ReturnCode {
        self(weechat, buffer, command)
    }
}

struct CommandRunHookData {
    callback: Box<dyn CommandRunCallback>,
    weechat_ptr: *mut t_weechat_plugin,
}

impl CommandRun {
    /// Override an existing Weechat command.
    ///
    /// # Arguments
    ///
    /// * `command` - The command to override (wildcard `*` is allowed).
    ///
    /// * `callback` - The function that will be called when the command is run.
    ///
    /// # Panics
    ///
    /// Panics if the method is not called from the main Weechat thread.
    ///
    /// # Example
    /// ```no_run
    /// # use std::borrow::Cow;
    /// # use weechat::{Weechat, ReturnCode};
    /// # use weechat::hooks::CommandRun;
    /// # use weechat::buffer::Buffer;
    ///
    /// let buffer_command = CommandRun::new(
    ///     "2000|/buffer *",
    ///     |_: &Weechat, _: &Buffer, _: Cow<str>| ReturnCode::OkEat,
    /// )
    /// .expect("Can't override buffer command");
    /// ```
    pub fn new(command: &str, callback: impl CommandRunCallback + 'static) -> Result<Self, ()> {
        unsafe extern "C" fn c_hook_cb(
            pointer: *const c_void,
            _data: *mut c_void,
            buffer: *mut t_gui_buffer,
            command: *const std::os::raw::c_char,
        ) -> c_int {
            let hook_data: &mut CommandRunHookData = { &mut *(pointer as *mut CommandRunHookData) };
            let cb = &mut hook_data.callback;

            let weechat = Weechat::from_ptr(hook_data.weechat_ptr);
            let buffer = weechat.buffer_from_ptr(buffer);
            let command = CStr::from_ptr(command).to_string_lossy();

            cb.callback(&weechat, &buffer, command) as isize as i32
        }

        Weechat::check_thread();
        let weechat = unsafe { Weechat::weechat() };

        let data = Box::new(CommandRunHookData {
            callback: Box::new(callback),
            weechat_ptr: weechat.ptr,
        });

        let data_ref = Box::leak(data);
        let hook_command_run = weechat.get().hook_command_run.unwrap();

        let command = LossyCString::new(command);

        let hook_ptr = unsafe {
            hook_command_run(
                weechat.ptr,
                command.as_ptr(),
                Some(c_hook_cb),
                data_ref as *const _ as *const c_void,
                ptr::null_mut(),
            )
        };
        let hook_data = unsafe { Box::from_raw(data_ref) };

        if hook_ptr.is_null() {
            Err(())
        } else {
            let hook = Hook {
                ptr: hook_ptr,
                weechat_ptr: weechat.ptr,
            };

            Ok(CommandRun {
                _hook: hook,
                _hook_data: hook_data,
            })
        }
    }
}

impl Command {
    /// Create a new Weechat command.
    ///
    /// Returns the hook of the command. The command is unhooked if the hook is
    /// dropped.
    ///
    /// # Arguments
    ///
    /// * `command_settings` - Settings for the new command.
    ///
    /// * `callback` - The callback that will be called if the command is run.
    ///
    /// ```no_run
    /// # use weechat::{Weechat, Args};
    /// # use weechat::hooks::{Command, CommandSettings};
    /// # use weechat::buffer::{Buffer};
    /// let settings = CommandSettings::new("irc")
    ///     .description("IRC chat protocol command.")
    ///     .add_argument("server add <server-name> <hostname>[:<port>]")
    ///     .add_argument("server delete|list|listfull <server-name>")
    ///     .add_argument("connect <server-name>")
    ///     .add_argument("disconnect <server-name>")
    ///     .add_argument("reconnect <server-name>")
    ///     .add_argument("help <irc-command> [<irc-subcommand>]")
    ///     .arguments_description(
    ///         "     server: List, add, or remove IRC servers.
    ///     connect: Connect to a IRC server.
    ///  disconnect: Disconnect from one or all IRC servers.
    ///   reconnect: Reconnect to server(s).
    ///        help: Show detailed command help.\n
    /// Use /irc [command] help to find out more.\n",
    ///     )
    ///     .add_completion("server |add|delete|list|listfull")
    ///     .add_completion("connect")
    ///     .add_completion("disconnect")
    ///     .add_completion("reconnect")
    ///     .add_completion("help server|connect|disconnect|reconnect");
    ///
    /// let command = Command::new(
    ///     settings,
    ///     |_: &Weechat, buffer: &Buffer, args: Args| {
    ///         buffer.print(&format!("Command called with args {:?}", args));
    ///     }
    /// ).expect("Can't create command");
    /// ```
    pub fn new(
        command_settings: CommandSettings,
        callback: impl CommandCallback + 'static,
    ) -> Result<Command, ()> {
        unsafe extern "C" fn c_hook_cb(
            pointer: *const c_void,
            _data: *mut c_void,
            buffer: *mut t_gui_buffer,
            argc: i32,
            argv: *mut *mut c_char,
            _argv_eol: *mut *mut c_char,
        ) -> c_int {
            let hook_data: &mut CommandHookData = { &mut *(pointer as *mut CommandHookData) };
            let weechat = Weechat::from_ptr(hook_data.weechat_ptr);
            let buffer = weechat.buffer_from_ptr(buffer);
            let cb = &mut hook_data.callback;
            let args = Args::new(argc, argv);

            cb.callback(&weechat, &buffer, args);

            WEECHAT_RC_OK
        }

        Weechat::check_thread();
        let weechat = unsafe { Weechat::weechat() };

        let name = LossyCString::new(command_settings.name);
        let description = LossyCString::new(command_settings.description);
        let args = LossyCString::new(command_settings.arguments.join("||"));
        let args_description = LossyCString::new(command_settings.argument_descriptoin);
        let completion = LossyCString::new(command_settings.completion.join("||"));

        let data = Box::new(CommandHookData {
            callback: Box::new(callback),
            weechat_ptr: weechat.ptr,
        });

        let data_ref = Box::leak(data);

        let hook_command = weechat.get().hook_command.unwrap();
        let hook_ptr = unsafe {
            hook_command(
                weechat.ptr,
                name.as_ptr(),
                description.as_ptr(),
                args.as_ptr(),
                args_description.as_ptr(),
                completion.as_ptr(),
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
            Ok(Command {
                _hook: hook,
                _hook_data: hook_data,
            })
        }
    }
}
