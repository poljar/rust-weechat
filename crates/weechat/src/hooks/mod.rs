//! Weechat Hook module.
//!
//! Weechat hooks are used for many different things, to create commands, to
//! listen to events on a file descriptor, add completions to Weechat, etc.

mod signal;

mod bar;
mod commands;
mod completion;
mod fd;
#[cfg(feature = "unsound")]
mod modifier;
mod timer;

pub use bar::{BarItem, BarItemCallback};
pub use commands::{Command, CommandCallback, CommandRun, CommandRunCallback, CommandSettings};
pub use completion::{Completion, CompletionCallback, CompletionHook, CompletionPosition};

pub use fd::{FdHook, FdHookCallback, FdHookMode};
#[cfg(feature = "unsound")]
pub use modifier::{ModifierCallback, ModifierData, ModifierHook};
pub use signal::{SignalCallback, SignalData, SignalHook};
pub use timer::{RemainingCalls, TimerCallback, TimerHook};

use crate::Weechat;
use weechat_sys::{t_hook, t_weechat_plugin};

/// Weechat Hook type. The hook is unhooked automatically when the object is
/// dropped.
pub(crate) struct Hook {
    pub(crate) ptr: *mut t_hook,
    pub(crate) weechat_ptr: *mut t_weechat_plugin,
}

impl Drop for Hook {
    fn drop(&mut self) {
        let weechat = Weechat::from_ptr(self.weechat_ptr);
        let unhook = weechat.get().unhook.unwrap();
        unsafe { unhook(self.ptr) };
    }
}
