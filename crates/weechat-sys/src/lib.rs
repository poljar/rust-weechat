#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(clippy::type_complexity)]
#![allow(clippy::redundant_static_lifetimes)]

use libc::c_int;

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

pub const WEECHAT_PLUGIN_API_VERSION_LENGTH: usize = 12;

/* return codes for plugin functions */
pub const WEECHAT_RC_OK: c_int = 0;
pub const WEECHAT_RC_OK_EAT: c_int = 1;
pub const WEECHAT_RC_ERROR: c_int = -1;

pub const WEECHAT_CONFIG_OPTION_SET_OK_CHANGED: c_int = 2;
pub const WEECHAT_CONFIG_OPTION_SET_OK_SAME_VALUE: c_int = 1;
pub const WEECHAT_CONFIG_OPTION_SET_ERROR: c_int = 0;
pub const WEECHAT_CONFIG_OPTION_SET_OPTION_NOT_FOUND: c_int = -1;

pub const WEECHAT_CONFIG_READ_OK: c_int = 0;
pub const WEECHAT_CONFIG_READ_MEMORY_ERROR: c_int = -1;
pub const WEECHAT_CONFIG_READ_FILE_NOT_FOUND: c_int = -2;
