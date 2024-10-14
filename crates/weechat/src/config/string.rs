use std::{borrow::Cow, ffi::CStr, marker::PhantomData};

use weechat_sys::{t_config_option, t_weechat_plugin};

use crate::{
    config::{
        config_options::{ConfigOptions, FromPtrs, HiddenConfigOptionT},
        BaseConfigOption, ConfigSection,
    },
    Weechat,
};

type StringCheckCb = Box<dyn FnMut(&Weechat, &StringOption, Cow<str>) -> bool>;
type StringChangeCallback = Box<dyn FnMut(&Weechat, &StringOption)>;

/// Settings for a new string option.
#[derive(Default)]
pub struct StringOptionSettings {
    pub(crate) name: String,

    pub(crate) description: String,

    pub(crate) default_value: String,

    pub(crate) change_cb: Option<StringChangeCallback>,

    pub(crate) check_cb: Option<StringCheckCb>,
}

impl StringOptionSettings {
    /// Create new settings that can be used to create a new string option.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the new option.
    pub fn new<N: Into<String>>(name: N) -> Self {
        StringOptionSettings { name: name.into(), ..Default::default() }
    }

    /// Set the description of the option.
    ///
    /// # Arguments
    ///
    /// * `description` - The description of the new option.
    pub fn description<D: Into<String>>(mut self, description: D) -> Self {
        self.description = description.into();
        self
    }

    /// Set the default value of the option.
    ///
    /// This is the value the option will have if it isn't set by the user. If
    /// the option is reset, the option will take this value.
    ///
    /// # Arguments
    ///
    /// * `value` - The value that should act as the default value.
    pub fn default_value<V: Into<String>>(mut self, value: V) -> Self {
        self.default_value = value.into();
        self
    }

    /// Set the callback that will run when the value of the option changes.
    ///
    /// # Arguments
    ///
    /// * `callback` - The callback that will be run.
    ///
    /// # Examples
    /// ```
    /// use weechat::Weechat;
    /// use weechat::config::StringOptionSettings;
    ///
    /// let settings = StringOptionSettings::new("address")
    ///     .set_change_callback(|weechat, option| {
    ///         Weechat::print("Option changed");
    ///     });
    /// ```
    pub fn set_change_callback(
        mut self,
        callback: impl FnMut(&Weechat, &StringOption) + 'static,
    ) -> Self {
        self.change_cb = Some(Box::new(callback));
        self
    }

    /// Set a callback to check the validity of the string option.
    ///
    /// # Arguments
    ///
    /// * `callback` - The callback that will be run.
    ///
    /// # Examples
    /// ```
    /// use weechat::config::StringOptionSettings;
    ///
    /// let settings = StringOptionSettings::new("address")
    ///     .set_check_callback(|weechat, option, value| {
    ///         value.starts_with("http")
    ///     });
    /// ```
    pub fn set_check_callback(
        mut self,
        callback: impl FnMut(&Weechat, &StringOption, Cow<str>) -> bool + 'static,
    ) -> Self {
        self.check_cb = Some(Box::new(callback));
        self
    }
}

/// A config option with a string value.
pub struct StringOption<'a> {
    pub(crate) ptr: *mut t_config_option,
    pub(crate) weechat_ptr: *mut t_weechat_plugin,
    pub(crate) _phantom: PhantomData<&'a ConfigSection>,
}

impl<'a> StringOption<'a> {
    /// Get the value of the option.
    pub fn value(&self) -> Cow<str> {
        let weechat = self.get_weechat();
        let config_string = weechat.get().config_string.unwrap();
        unsafe {
            let string = config_string(self.get_ptr());
            CStr::from_ptr(string).to_string_lossy()
        }
    }
}

impl<'a> FromPtrs for StringOption<'a> {
    fn from_ptrs(option_ptr: *mut t_config_option, weechat_ptr: *mut t_weechat_plugin) -> Self {
        StringOption { ptr: option_ptr, weechat_ptr, _phantom: PhantomData }
    }
}

impl<'a> HiddenConfigOptionT for StringOption<'a> {
    fn get_ptr(&self) -> *mut t_config_option {
        self.ptr
    }

    fn get_weechat(&self) -> Weechat {
        Weechat::from_ptr(self.weechat_ptr)
    }
}

impl<'a> BaseConfigOption for StringOption<'a> {}
impl<'a> ConfigOptions for StringOption<'a> {}
