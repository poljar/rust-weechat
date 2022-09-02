//! Weechat configuration for plugins.
//!
//! # Examples
//!
//! ```no_run
//! use weechat::Weechat;
//! use weechat::config::{Config, BooleanOptionSettings, ConfigSectionSettings, BooleanOption};
//!
//! let mut config = Config::new("my_plugin")
//!     .expect("Can't create new config");
//!
//! let server_section_options = ConfigSectionSettings::new("look");
//! {
//!     let mut look_section = config.new_section(server_section_options)
//!         .expect("Can't create new section");
//!
//!     let use_colors = BooleanOptionSettings::new("use_colors")
//!         .set_change_callback(move |weechat: &Weechat, option: &BooleanOption| {});
//!
//!     let use_colors = look_section.new_boolean_option(use_colors);
//! }
//!
//! config.read().expect("Can't read config");
//!
//! ```

mod boolean;
mod color;
#[allow(clippy::module_inception)]
mod config;
mod config_options;
mod integer;
mod section;
mod string;

pub use crate::config::{
    boolean::{BooleanOption, BooleanOptionSettings},
    color::{ColorOption, ColorOptionSettings},
    config::{Conf, Config, ConfigReloadCallback, OptionChanged},
    integer::{IntegerOption, IntegerOptionSettings},
    string::{StringOption, StringOptionSettings},
};

pub use crate::config::{
    config_options::{BaseConfigOption, ConfigOptions, OptionType},
    section::{
        ConfigOption, ConfigSection, ConfigSectionSettings, SectionHandle, SectionHandleMut,
        SectionReadCallback, SectionWriteCallback, SectionWriteDefaultCallback,
    },
};
