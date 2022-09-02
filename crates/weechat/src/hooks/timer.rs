use libc::c_int;
use std::{os::raw::c_void, ptr, time::Duration};

use weechat_sys::{t_weechat_plugin, WEECHAT_RC_OK};

use super::Hook;
use crate::Weechat;

/// A hook for a timer, the hook will be removed when the object is dropped.
pub struct TimerHook {
    _hook: Hook,
    _hook_data: Box<TimerHookData>,
}

/// Enum representing how many calls a timer still has.
pub enum RemainingCalls {
    /// Infinitely many remaining calls.
    Infinite,
    /// A finite number of calls is remaining.
    Finite(i32),
}

impl From<i32> for RemainingCalls {
    fn from(remaining: i32) -> Self {
        match remaining {
            -1 => RemainingCalls::Infinite,
            r => RemainingCalls::Finite(r),
        }
    }
}

/// Trait for the timer callback
///
/// A blanket implementation for pure `FnMut` functions exists, if data needs to
/// be passed to the callback implement this over your struct.
pub trait TimerCallback {
    /// Callback that will be called when the timer fires.
    ///
    /// # Arguments
    ///
    /// * `weechat` - A Weechat context.
    ///
    /// * `remaining_calls` - How many times the timer will fire.
    fn callback(&mut self, weechat: &Weechat, remaining_calls: RemainingCalls);
}

impl<T: FnMut(&Weechat, RemainingCalls) + 'static> TimerCallback for T {
    fn callback(&mut self, weechat: &Weechat, remaining_calls: RemainingCalls) {
        self(weechat, remaining_calls)
    }
}

struct TimerHookData {
    callback: Box<dyn TimerCallback>,
    weechat_ptr: *mut t_weechat_plugin,
}

impl TimerHook {
    /// Create a timer that will repeatedly fire.
    ///
    /// # Arguments
    ///
    /// * `interval` - The delay between calls in milliseconds.
    ///
    /// * `align_second` - The alignment on a second. For example, if the
    ///     current time is 09:00, if the interval = 60000 (60 seconds), and
    ///     align_second = 60, then timer is called each minute on the 0th
    ///     second.
    ///
    /// * `max_calls` - The number of times the callback should be called, 0
    ///     means it's called forever.
    ///
    /// * `callback` - A function that will be called when the timer fires, the
    ///     `remaining` argument will be -1 if the timer has no end.
    ///
    /// # Panics
    ///
    /// Panics if the method is not called from the main Weechat thread.
    ///
    /// # Example
    /// ```no_run
    /// # use std::time::Duration;
    /// # use weechat::{Weechat};
    /// # use weechat::hooks::{TimerHook, RemainingCalls};
    ///
    /// let timer = TimerHook::new(
    ///     Duration::from_secs(1), 0, -1,
    ///     |_: &Weechat, _: RemainingCalls| {
    ///         Weechat::print("Running timer hook");
    ///     }
    /// ).expect("Can't create timer hook");
    /// ```
    pub fn new(
        interval: Duration,
        align_second: i32,
        max_calls: i32,
        callback: impl TimerCallback + 'static,
    ) -> Result<TimerHook, ()> {
        unsafe extern "C" fn c_hook_cb(
            pointer: *const c_void,
            _data: *mut c_void,
            remaining: i32,
        ) -> c_int {
            let hook_data: &mut TimerHookData = { &mut *(pointer as *mut TimerHookData) };
            let cb = &mut hook_data.callback;

            cb.callback(
                &Weechat::from_ptr(hook_data.weechat_ptr),
                RemainingCalls::from(remaining),
            );

            WEECHAT_RC_OK
        }

        Weechat::check_thread();
        let weechat = unsafe { Weechat::weechat() };

        let data = Box::new(TimerHookData {
            callback: Box::new(callback),
            weechat_ptr: weechat.ptr,
        });

        let data_ref = Box::leak(data);
        let hook_timer = weechat.get().hook_timer.unwrap();

        let hook_ptr = unsafe {
            hook_timer(
                weechat.ptr,
                interval.as_millis() as i64,
                align_second,
                max_calls,
                Some(c_hook_cb),
                data_ref as *const _ as *const c_void,
                ptr::null_mut(),
            )
        };
        let hook_data = unsafe { Box::from_raw(data_ref) };

        if hook_ptr.is_null() {
            Err(())
        } else {
            Ok(TimerHook {
                _hook: Hook {
                    ptr: hook_ptr,
                    weechat_ptr: weechat.ptr,
                },
                _hook_data: hook_data,
            })
        }
    }
}
