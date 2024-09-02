use std::marker::PhantomData;

use weechat_sys::{t_config_option, t_weechat_plugin};

use crate::{
    config::{
        config_options::{ConfigOptions, FromPtrs, HiddenConfigOptionT},
        BaseConfigOption, ConfigSection,
    },
    Weechat,
};

/// Settings for a new string option.
#[derive(Default)]
pub struct EnumOptionSettings {
    pub(crate) name: String,

    pub(crate) description: String,

    pub(crate) default_value: i32,

    pub(crate) min: i32,

    pub(crate) max: i32,

    pub(crate) string_values: String,

    pub(crate) change_cb: Option<Box<dyn FnMut(&Weechat, &EnumOption)>>,
}

impl EnumOptionSettings {
    /// Create new settings that can be used to create a new string option.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the new option.
    pub fn new<N: Into<String>>(name: N) -> Self {
        EnumOptionSettings { name: name.into(), ..Default::default() }
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
    pub fn default_value<V: Into<i32>>(mut self, value: V) -> Self {
        self.default_value = value.into();
        self
    }

    /// Set minimal value of the integer option.
    ///
    /// # Arguments
    ///
    /// * `value` - The values that should act as minimal valid value.
    pub fn min(mut self, value: i32) -> Self {
        self.min = value;
        self
    }

    /// Set maximum value of the integer option.
    ///
    /// # Arguments
    ///
    /// * `value` - The values that should act as maximal valid value.
    pub fn max(mut self, value: i32) -> Self {
        self.max = value;
        self
    }

    /// Set the string values of the option.
    ///
    /// This setting decides if the integer option should act as an enum taking
    /// symbolic values.
    ///
    /// # Arguments
    ///
    /// * `values` - The values that should act as the symbolic values.
    ///
    /// # Examples
    /// ```no_run
    /// use weechat::config::IntegerOptionSettings;
    ///
    /// let settings = IntegerOptionSettings::new("server_buffer")
    ///     .string_values(vec!["independent", "merged"]);
    /// ```
    pub fn string_values<I, T>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<String>,
    {
        let vec: Vec<String> = values.into_iter().map(Into::into).collect();
        self.string_values = vec.join("|");
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
    /// use weechat::config::EnumOptionSettings;
    ///
    /// let settings = EnumOptionSettings::new("address")
    ///     .set_change_callback(|weechat, option| {
    ///         Weechat::print("Option changed");
    ///     });
    /// ```
    pub fn set_change_callback(
        mut self,
        callback: impl FnMut(&Weechat, &EnumOption) + 'static,
    ) -> Self {
        self.change_cb = Some(Box::new(callback));
        self
    }
}

/// A config option with a string value.
pub struct EnumOption<'a> {
    pub(crate) ptr: *mut t_config_option,
    pub(crate) weechat_ptr: *mut t_weechat_plugin,
    pub(crate) _phantom: PhantomData<&'a ConfigSection>,
}

impl<'a> EnumOption<'a> {
    /// Get the value of the option.
    pub fn value(&self) -> i32 {
        let weechat = self.get_weechat();
        let config_enum = weechat.get().config_enum.unwrap();
        unsafe { config_enum(self.get_ptr()) }
    }
}

impl<'a> FromPtrs for EnumOption<'a> {
    fn from_ptrs(option_ptr: *mut t_config_option, weechat_ptr: *mut t_weechat_plugin) -> Self {
        EnumOption { ptr: option_ptr, weechat_ptr, _phantom: PhantomData }
    }
}

impl<'a> HiddenConfigOptionT for EnumOption<'a> {
    fn get_ptr(&self) -> *mut t_config_option {
        self.ptr
    }

    fn get_weechat(&self) -> Weechat {
        Weechat::from_ptr(self.weechat_ptr)
    }
}

impl<'a> BaseConfigOption for EnumOption<'a> {}
impl<'a> ConfigOptions for EnumOption<'_> {}
