//! # `weechat`
//!
//! This crate implements high level bindings for the Weechat plugin API.
//!
//! The bindings make it possible to create powerful Weechat plugins using Rust.
//!
//! ```no_run
//! use weechat::{
//!    buffer::Buffer,
//!    plugin, Args, Weechat, Plugin,
//! };
//!
//! struct HelloWorld;
//!
//! impl Plugin for HelloWorld {
//!     fn init(_: &Weechat, _: Args) -> Result<Self, ()> {
//!         Weechat::print("Hello from Rust");
//!         Ok(Self)
//!     }
//! }
//!
//! impl Drop for HelloWorld {
//!     fn drop(&mut self) {
//!         Weechat::print("Bye from Rust");
//!     }
//! }
//!
//! plugin!(
//!     HelloWorld,
//!     name: "hello",
//!     author: "Damir Jelić <poljar@termina.org.uk>",
//!     description: "Simple hello world Rust plugin",
//!     version: "1.0.0",
//!     license: "MIT"
//! );
//! ```

#![deny(missing_docs)]
#![allow(clippy::result_unit_err)]
#![cfg_attr(feature = "docs", feature(doc_cfg))]

use std::ffi::CString;

#[cfg(feature = "async")]
mod executor;
mod hashtable;
mod hdata;
mod weechat;

#[cfg(feature = "config_macro")]
#[macro_use]
mod config_macros;

#[cfg(feature = "config_macro")]
pub use paste;
#[cfg(feature = "config_macro")]
pub use strum;

pub mod buffer;
pub mod config;
pub mod hooks;
pub mod infolist;

pub use crate::weechat::{Args, Prefix, Weechat};

pub use libc;
pub use weechat_macro::plugin;
pub use weechat_sys;

/// Weechat plugin trait.
///
/// Implement this trait over your struct to implement a Weechat plugin. The
/// init method will get called when Weechat loads the plugin, while the
///
/// Drop method will be called when Weechat unloads the plugin.
pub trait Plugin: Sized {
    /// The initialization method for the plugin.
    ///
    /// This will be called when Weechat loads the pluign.
    ///
    /// # Arguments
    ///
    /// * `weechat` - A borrow to a Weechat object that will be valid during the
    ///     duration of the init callback.
    ///
    /// * `args` - Arguments passed to the plugin when it is loaded.
    fn init(weechat: &Weechat, args: Args) -> Result<Self, ()>;
}

#[cfg(feature = "async")]
#[cfg_attr(feature = "docs", doc(cfg(r#async)))]
pub use executor::Task;

/// Status values for Weechat callbacks
pub enum ReturnCode {
    /// The callback returned successfully.
    Ok = weechat_sys::WEECHAT_RC_OK as isize,
    /// The callback returned successfully and the command will not be executed
    /// after the callback.
    OkEat = weechat_sys::WEECHAT_RC_OK_EAT as isize,
    /// The callback returned with an error.
    Error = weechat_sys::WEECHAT_RC_ERROR as isize,
}

pub(crate) struct LossyCString;

impl LossyCString {
    #[allow(clippy::new_ret_no_self)]
    pub(crate) fn new<T: AsRef<str>>(t: T) -> CString {
        match CString::new(t.as_ref()) {
            Ok(cstr) => cstr,
            Err(_) => CString::new(t.as_ref().replace('\0', "")).expect("string has no nulls"),
        }
    }
}
