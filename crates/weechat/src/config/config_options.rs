use crate::{config::OptionChanged, LossyCString, Weechat};
use std::{borrow::Cow, convert::TryFrom, ffi::CStr};
use weechat_sys::{t_config_option, t_weechat_plugin};

#[derive(Debug, PartialEq, Clone)]
#[allow(missing_docs)]
pub enum OptionType {
    Boolean,
    Integer,
    String,
    Color,
}

impl TryFrom<&str> for OptionType {
    type Error = &'static str;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let ret = match value {
            "boolean" => OptionType::Boolean,
            "integer" => OptionType::Integer,
            "string" => OptionType::String,
            "color" => OptionType::Color,
            _ => return Err("Invalid option type"),
        };

        Ok(ret)
    }
}

impl OptionType {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            OptionType::Boolean => "boolean",
            OptionType::Integer => "integer",
            OptionType::String => "string",
            OptionType::Color => "color",
        }
    }
}

impl Default for OptionType {
    fn default() -> Self {
        OptionType::String
    }
}

pub trait FromPtrs {
    /// Returns the raw pointer to the config option.
    fn from_ptrs(option_ptr: *mut t_config_option, weechat_ptr: *mut t_weechat_plugin) -> Self;
}

pub trait HidenConfigOptionT {
    /// Returns the raw pointer to the config option.
    fn get_ptr(&self) -> *mut t_config_option;
    fn get_weechat(&self) -> Weechat;

    fn get_string(&self, property: &str) -> Option<Cow<str>> {
        let weechat = self.get_weechat();
        let get_string = weechat.get().config_option_get_string.unwrap();
        let property = LossyCString::new(property);

        unsafe {
            let string = get_string(self.get_ptr(), property.as_ptr());
            if string.is_null() {
                None
            } else {
                Some(CStr::from_ptr(string).to_string_lossy())
            }
        }
    }
}

/// Base configuration option methods.
///
/// These methods are implemented for every option and don't depend on the
/// option type.
pub trait BaseConfigOption: HidenConfigOptionT {
    /// Get the name of the option.
    fn name(&self) -> Cow<str> {
        self.get_string("name")
            .expect("Can't get the name of the option")
    }

    /// Get the description of the option.
    fn description(&self) -> Cow<str> {
        self.get_string("description")
            .expect("Can't get the description of the option")
    }

    /// Get the section name of the section the option belongs to.
    fn section_name(&self) -> Cow<str> {
        self.get_string("section_name")
            .expect("Can't get the section name of the option")
    }

    /// Get the config name the option belongs to.
    fn config_name(&self) -> Cow<str> {
        self.get_string("config_name")
            .expect("Can't get the config name of the option")
    }

    /// Get the type of the config option
    fn option_type(&self) -> OptionType {
        let option_type = self
            .get_string("type")
            .expect("Can't get the config name of the option");
        OptionType::try_from(option_type.as_ref()).unwrap()
    }

    /// Resets the option to its default value.
    fn reset(&self, run_callback: bool) -> OptionChanged {
        let weechat = self.get_weechat();
        let option_reset = weechat.get().config_option_reset.unwrap();

        let ret = unsafe { option_reset(self.get_ptr(), run_callback as i32) };

        OptionChanged::from_int(ret)
    }

    /// Set the option using a string.
    ///
    /// Weechat will parse the string and turn it into a appropriate value
    /// depending on the option type.
    ///
    /// # Arguments
    /// `value` - The value to which the option should be set.
    fn set(&self, value: &str, run_callback: bool) -> OptionChanged {
        let value = LossyCString::new(value);

        let weechat = self.get_weechat();
        let option_set = weechat.get().config_option_set.unwrap();

        let ret = unsafe { option_set(self.get_ptr(), value.as_ptr(), run_callback as i32) };

        OptionChanged::from_int(ret)
    }

    /// Is the option undefined/null.
    fn is_null(&self) -> bool {
        let weechat = self.get_weechat();
        let is_null = weechat.get().config_option_is_null.unwrap();

        let ret = unsafe { is_null(self.get_ptr()) };

        ret != 0
    }
}

/// Marker trait for config options.
pub trait ConfigOptions: BaseConfigOption + FromPtrs {}

pub(crate) type CheckCB<T> = dyn FnMut(&Weechat, &T, Cow<str>) -> bool;

pub(crate) struct OptionPointers<T> {
    pub(crate) weechat_ptr: *mut t_weechat_plugin,
    pub(crate) check_cb: Option<Box<CheckCB<T>>>,
    pub(crate) change_cb: Option<Box<dyn FnMut(&Weechat, &T)>>,
    pub(crate) delete_cb: Option<Box<dyn FnMut(&Weechat, &T)>>,
}
