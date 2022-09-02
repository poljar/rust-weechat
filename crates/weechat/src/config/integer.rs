use crate::{
    config::{
        config_options::{ConfigOptions, FromPtrs, HidenConfigOptionT},
        BaseConfigOption, ConfigSection,
    },
    Weechat,
};
use std::marker::PhantomData;
use weechat_sys::{t_config_option, t_weechat_plugin};

/// Settings for a new integer option.
#[derive(Default)]
pub struct IntegerOptionSettings {
    pub(crate) name: String,

    pub(crate) description: String,

    pub(crate) default_value: i32,

    pub(crate) min: i32,

    pub(crate) max: i32,

    pub(crate) string_values: String,

    pub(crate) change_cb: Option<Box<dyn FnMut(&Weechat, &IntegerOption)>>,
}

impl IntegerOptionSettings {
    /// Create new settings that can be used to create a new integer option.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the new option.
    pub fn new<N: Into<String>>(name: N) -> Self {
        IntegerOptionSettings {
            name: name.into(),
            ..Default::default()
        }
    }

    /// Set the description of the option.
    ///
    /// # Arguments
    ///
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
    pub fn default_value<V: Into<i32>>(mut self, value: V) -> Self {
        self.default_value = value.into();
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

    /// Set the callback that will run when the value of the option changes.
    ///
    /// # Arguments
    ///
    /// * `callback` - The callback that will be run.
    ///
    /// # Examples
    ///
    /// ```
    /// use weechat::Weechat;
    /// use weechat::config::IntegerOptionSettings;
    ///
    /// let settings = IntegerOptionSettings::new("server_buffer")
    ///     .string_values(vec!["independent", "merged"])
    ///     .set_change_callback(|weechat, option| {
    ///         Weechat::print("Option changed");
    ///     });
    /// ```
    pub fn set_change_callback(
        mut self,
        callback: impl FnMut(&Weechat, &IntegerOption) + 'static,
    ) -> Self {
        self.change_cb = Some(Box::new(callback));
        self
    }
}

/// A config option with a integer value.
pub struct IntegerOption<'a> {
    pub(crate) ptr: *mut t_config_option,
    pub(crate) weechat_ptr: *mut t_weechat_plugin,
    pub(crate) _phantom: PhantomData<&'a ConfigSection>,
}

impl<'a> IntegerOption<'a> {
    /// Get the value of the option.
    pub fn value(&self) -> i32 {
        let weechat = self.get_weechat();
        let config_integer = weechat.get().config_integer.unwrap();
        unsafe { config_integer(self.get_ptr()) }
    }
}

impl<'a> FromPtrs for IntegerOption<'a> {
    fn from_ptrs(option_ptr: *mut t_config_option, weechat_ptr: *mut t_weechat_plugin) -> Self {
        IntegerOption {
            ptr: option_ptr,
            weechat_ptr,
            _phantom: PhantomData,
        }
    }
}

impl<'a> HidenConfigOptionT for IntegerOption<'a> {
    fn get_ptr(&self) -> *mut t_config_option {
        self.ptr
    }

    fn get_weechat(&self) -> Weechat {
        Weechat::from_ptr(self.weechat_ptr)
    }
}

impl<'a> BaseConfigOption for IntegerOption<'a> {}
impl<'a> ConfigOptions for IntegerOption<'_> {}
