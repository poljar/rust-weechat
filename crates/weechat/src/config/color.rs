use crate::{
    config::{
        config_options::{ConfigOptions, FromPtrs, HidenConfigOptionT},
        BaseConfigOption, ConfigSection,
    },
    Weechat,
};
use std::{borrow::Cow, ffi::CStr, marker::PhantomData};
use weechat_sys::{t_config_option, t_weechat_plugin};

/// Settings for a new color option.
#[derive(Default)]
pub struct ColorOptionSettings {
    pub(crate) name: String,

    pub(crate) description: String,

    pub(crate) default_value: String,

    pub(crate) change_cb: Option<Box<dyn FnMut(&Weechat, &ColorOption)>>,
}

impl ColorOptionSettings {
    /// Create new settings that can be used to create a new color option.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the new option.
    pub fn new<N: Into<String>>(name: N) -> Self {
        ColorOptionSettings {
            name: name.into(),
            ..Default::default()
        }
    }

    /// Set the description of the option.
    ///
    /// # Arguments
    /// * `description` - The description of the new option.
    pub fn description<D: Into<String>>(mut self, descritpion: D) -> Self {
        self.description = descritpion.into();
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
    /// use weechat::config::ColorOptionSettings;
    ///
    /// let settings = ColorOptionSettings::new("address")
    ///     .set_change_callback(|weechat, option| {
    ///         Weechat::print("Option changed");
    ///     });
    /// ```
    pub fn set_change_callback(
        mut self,
        callback: impl FnMut(&Weechat, &ColorOption) + 'static,
    ) -> Self {
        self.change_cb = Some(Box::new(callback));
        self
    }
}

/// A config option with a color value.
pub struct ColorOption<'a> {
    pub(crate) ptr: *mut t_config_option,
    pub(crate) weechat_ptr: *mut t_weechat_plugin,
    pub(crate) _phantom: PhantomData<&'a ConfigSection>,
}

impl<'a> ColorOption<'a> {
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

impl<'a> FromPtrs for ColorOption<'a> {
    fn from_ptrs(option_ptr: *mut t_config_option, weechat_ptr: *mut t_weechat_plugin) -> Self {
        ColorOption {
            ptr: option_ptr,
            weechat_ptr,
            _phantom: PhantomData,
        }
    }
}

impl<'a> HidenConfigOptionT for ColorOption<'a> {
    fn get_ptr(&self) -> *mut t_config_option {
        self.ptr
    }

    fn get_weechat(&self) -> Weechat {
        Weechat::from_ptr(self.weechat_ptr)
    }
}

impl<'a> BaseConfigOption for ColorOption<'a> {}
impl<'a> ConfigOptions for ColorOption<'_> {}
