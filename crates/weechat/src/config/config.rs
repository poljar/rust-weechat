use std::{
    borrow::Cow,
    cell::RefCell,
    collections::HashMap,
    ffi::CStr,
    io::{Error as IoError, ErrorKind},
    marker::PhantomData,
    os::raw::{c_char, c_int, c_void},
    ptr,
    rc::Rc,
};

use weechat_sys::{
    t_config_file, t_config_option, t_config_section, t_weechat_plugin, WEECHAT_RC_OK,
};

use crate::{
    config::{
        section::{
            ConfigSection, ConfigSectionPointers, ConfigSectionSettings, SectionHandle,
            SectionHandleMut, SectionReadCbT, SectionWriteCbT,
        },
        BaseConfigOption, BooleanOption, ColorOption, ConfigOption, IntegerOption, StringOption,
    },
    LossyCString, Weechat,
};

/// Weechat configuration file
pub struct Config {
    inner: Conf,
    _config_data: *mut ConfigPointers,
    sections: HashMap<String, Rc<RefCell<ConfigSection>>>,
}

/// The borrowed equivalent of the `Config`. Will be present in callbacks.
pub struct Conf {
    ptr: *mut t_config_file,
    weechat_ptr: *mut t_weechat_plugin,
}

/// Status for updating options
#[derive(Debug)]
pub enum OptionChanged {
    /// The option was successfully changed.
    Changed = weechat_sys::WEECHAT_CONFIG_OPTION_SET_OK_CHANGED as isize,
    /// The options value has not changed.
    Unchanged = weechat_sys::WEECHAT_CONFIG_OPTION_SET_OK_SAME_VALUE as isize,
    /// The option was not found.
    NotFound = weechat_sys::WEECHAT_CONFIG_OPTION_SET_OPTION_NOT_FOUND as isize,
    /// An error occurred changing the value.
    Error = weechat_sys::WEECHAT_CONFIG_OPTION_SET_ERROR as isize,
}

impl OptionChanged {
    pub(crate) fn from_int(v: i32) -> OptionChanged {
        use OptionChanged::*;
        match v {
            weechat_sys::WEECHAT_CONFIG_OPTION_SET_OK_CHANGED => Changed,
            weechat_sys::WEECHAT_CONFIG_OPTION_SET_OK_SAME_VALUE => Unchanged,
            weechat_sys::WEECHAT_CONFIG_OPTION_SET_OPTION_NOT_FOUND => NotFound,
            weechat_sys::WEECHAT_CONFIG_OPTION_SET_ERROR => Error,
            _ => unreachable!(),
        }
    }
}

struct ConfigPointers {
    reload_cb: Option<Box<dyn ConfigReloadCallback>>,
    weechat_ptr: *mut t_weechat_plugin,
}

type ReloadCB = unsafe extern "C" fn(
    pointer: *const c_void,
    _data: *mut c_void,
    config_pointer: *mut t_config_file,
) -> c_int;

/// Trait for the config reload callback.
///
/// This trait can be implemented or a normal function or coroutine can be
/// passed as the callback.
pub trait ConfigReloadCallback: 'static {
    /// Function called when configuration file is reloaded with /reload
    ///
    /// # Arguments
    ///
    /// * `weeechat` - A reference to the weechat context.
    ///
    /// * `config` - A reference to the non-owned config.
    fn callback(&mut self, weechat: &Weechat, config: &Conf);
}

impl<T: FnMut(&Weechat, &Conf) + 'static> ConfigReloadCallback for T {
    fn callback(&mut self, weechat: &Weechat, config: &Conf) {
        self(weechat, config)
    }
}

impl Weechat {
    pub(crate) fn config_option_get_string(
        &self,
        pointer: *mut t_config_option,
        property: &str,
    ) -> Option<Cow<str>> {
        let get_string = self.get().config_option_get_string.unwrap();
        let property = LossyCString::new(property);

        unsafe {
            let string = get_string(pointer, property.as_ptr());
            if string.is_null() {
                None
            } else {
                Some(CStr::from_ptr(string).to_string_lossy())
            }
        }
    }

    /// Search an option with a full name.
    /// # Arguments
    ///
    /// * `option_name` - The full name of the option that should be searched for
    /// (format: "file.section.option").
    pub fn config_get(&self, option_name: &str) -> Option<ConfigOption> {
        let weechat = Weechat::from_ptr(self.ptr);
        let config_get = weechat.get().config_get.unwrap();
        let name = LossyCString::new(option_name);

        let ptr = unsafe { config_get(name.as_ptr()) };

        if ptr.is_null() {
            return None;
        }

        let option_type = weechat.config_option_get_string(ptr, "type").unwrap();

        Some(Config::option_from_type_and_ptr(
            self.ptr,
            ptr,
            option_type.as_ref(),
        ))
    }

    /// Get value of a plugin option
    pub fn get_plugin_option(&self, option: &str) -> Option<Cow<str>> {
        let config_get_plugin = self.get().config_get_plugin.unwrap();

        let option_name = LossyCString::new(option);

        unsafe {
            let option = config_get_plugin(self.ptr, option_name.as_ptr());
            if option.is_null() {
                None
            } else {
                Some(CStr::from_ptr(option).to_string_lossy())
            }
        }
    }

    /// Set the value of a plugin option
    pub fn set_plugin_option(&self, option: &str, value: &str) -> OptionChanged {
        let config_set_plugin = self.get().config_set_plugin.unwrap();

        let option_name = LossyCString::new(option);
        let value = LossyCString::new(value);

        unsafe {
            let result = config_set_plugin(self.ptr, option_name.as_ptr(), value.as_ptr());

            OptionChanged::from_int(result as i32)
        }
    }
}

impl Drop for Config {
    fn drop(&mut self) {
        let weechat = Weechat::from_ptr(self.inner.weechat_ptr);
        let config_free = weechat.get().config_free.unwrap();

        // Drop the sections first.
        self.sections.clear();

        unsafe {
            // Now drop the config.
            Box::from_raw(self._config_data);
            config_free(self.inner.ptr)
        };
    }
}

impl Config {
    /// Create a new Weechat configuration file, returns a `Config` object.
    /// The configuration file is freed when the `Config` object is dropped.
    ///
    /// # Arguments
    /// * `name` - Name of the new configuration file
    ///
    /// # Panics
    ///
    /// Panics if the method is not called from the main Weechat thread.
    pub fn new(name: &str) -> Result<Config, ()> {
        Config::config_new_helper(name, None)
    }

    /// Create a new Weechat configuration file, returns a `Config` object.
    /// The configuration file is freed when the `Config` object is dropped.
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the new configuration file
    ///
    /// * `reload_callback` - Callback that will be called when the
    /// configuration file is reloaded.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use weechat::Weechat;
    /// use weechat::config::{Conf, Config};
    ///
    /// let config = Config::new_with_callback("server_buffer",
    ///     |weechat: &Weechat, conf: &Conf| {
    ///         Weechat::print("Config was reloaded");
    ///     }
    /// );
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if the method is not called from the main Weechat thread.
    pub fn new_with_callback(
        name: &str,
        reload_callback: impl ConfigReloadCallback,
    ) -> Result<Config, ()> {
        let callback = Box::new(reload_callback);
        Config::config_new_helper(name, Some(callback))
    }

    fn config_new_helper(
        name: &str,
        callback: Option<Box<dyn ConfigReloadCallback>>,
    ) -> Result<Config, ()> {
        unsafe extern "C" fn c_reload_cb(
            pointer: *const c_void,
            _data: *mut c_void,
            config_pointer: *mut t_config_file,
        ) -> c_int {
            let pointers: &mut ConfigPointers = { &mut *(pointer as *mut ConfigPointers) };

            let cb = &mut pointers
                .reload_cb
                .as_mut()
                .expect("C callback was set while no rust callback");
            let conf = Conf {
                ptr: config_pointer,
                weechat_ptr: pointers.weechat_ptr,
            };

            let weechat = Weechat::from_ptr(pointers.weechat_ptr);

            cb.callback(&weechat, &conf);

            WEECHAT_RC_OK
        }

        Weechat::check_thread();
        let weechat = unsafe { Weechat::weechat() };

        let c_name = LossyCString::new(name);

        let c_reload_cb = match callback {
            Some(_) => Some(c_reload_cb as ReloadCB),
            None => None,
        };

        let config_pointers = Box::new(ConfigPointers {
            reload_cb: callback,
            weechat_ptr: weechat.ptr,
        });
        let config_pointers_ref = Box::leak(config_pointers);

        let config_new = weechat.get().config_new.unwrap();

        let config_ptr = unsafe {
            config_new(
                weechat.ptr,
                c_name.as_ptr(),
                c_reload_cb,
                config_pointers_ref as *const _ as *const c_void,
                ptr::null_mut(),
            )
        };

        if config_ptr.is_null() {
            unsafe { Box::from_raw(config_pointers_ref) };
            return Err(());
        };

        Ok(Config {
            inner: Conf {
                ptr: config_ptr,
                weechat_ptr: weechat.ptr,
            },
            _config_data: config_pointers_ref,
            sections: HashMap::new(),
        })
    }

    pub(crate) fn option_from_type_and_ptr<'a>(
        weechat_ptr: *mut t_weechat_plugin,
        option_ptr: *mut t_config_option,
        option_type: &str,
    ) -> ConfigOption<'a> {
        match option_type {
            "boolean" => ConfigOption::Boolean(BooleanOption {
                ptr: option_ptr,
                weechat_ptr,
                _phantom: PhantomData,
            }),
            "integer" => ConfigOption::Integer(IntegerOption {
                ptr: option_ptr,
                weechat_ptr,
                _phantom: PhantomData,
            }),
            "string" => ConfigOption::String(StringOption {
                ptr: option_ptr,
                weechat_ptr,
                _phantom: PhantomData,
            }),
            "color" => ConfigOption::Color(ColorOption {
                ptr: option_ptr,
                weechat_ptr,
                _phantom: PhantomData,
            }),
            _ => unreachable!(),
        }
    }
    fn return_value_to_error(return_value: c_int) -> std::io::Result<()> {
        match return_value {
            weechat_sys::WEECHAT_CONFIG_READ_OK => Ok(()),
            weechat_sys::WEECHAT_CONFIG_READ_FILE_NOT_FOUND => {
                Err(IoError::new(ErrorKind::NotFound, "File was not found"))
            }
            weechat_sys::WEECHAT_CONFIG_READ_MEMORY_ERROR => {
                Err(IoError::new(ErrorKind::Other, "Not enough memory"))
            }
            _ => unreachable!(),
        }
    }

    /// Read the configuration file from the disk.
    pub fn read(&self) -> std::io::Result<()> {
        let weechat = Weechat::from_ptr(self.inner.weechat_ptr);
        let config_read = weechat.get().config_read.unwrap();

        let ret = unsafe { config_read(self.inner.ptr) };

        Config::return_value_to_error(ret)
    }

    /// Write the configuration file to the disk.
    pub fn write(&self) -> std::io::Result<()> {
        let weechat = Weechat::from_ptr(self.inner.weechat_ptr);
        let config_write = weechat.get().config_write.unwrap();

        let ret = unsafe { config_write(self.inner.ptr) };

        Config::return_value_to_error(ret)
    }

    /// Create a new section in the configuration file.
    ///
    /// # Arguments
    ///
    /// * `section_settings` - Settings that decide how the section will be
    /// created.
    ///
    /// # Panics
    ///
    /// Panics if the method is not called from the main Weechat thread.
    pub fn new_section(
        &mut self,
        section_settings: ConfigSectionSettings,
    ) -> Result<SectionHandleMut, ()> {
        unsafe extern "C" fn c_read_cb(
            pointer: *const c_void,
            _data: *mut c_void,
            config: *mut t_config_file,
            _section: *mut t_config_section,
            option_name: *const c_char,
            value: *const c_char,
        ) -> c_int {
            let option_name = CStr::from_ptr(option_name).to_string_lossy();
            let value = CStr::from_ptr(value).to_string_lossy();
            let pointers: &mut ConfigSectionPointers =
                { &mut *(pointer as *mut ConfigSectionPointers) };

            let conf = Conf {
                ptr: config,
                weechat_ptr: pointers.weechat_ptr,
            };
            let section = pointers
                .section
                .as_ref()
                .expect("Section reference wasn't set up correctly")
                .upgrade()
                .expect("Config has been destroyed but a read callback run");

            let weechat = Weechat::from_ptr(pointers.weechat_ptr);

            let cb = pointers
                .read_cb
                .as_mut()
                .expect("C read callback was called but no ruts callback");

            let ret = cb.callback(
                &weechat,
                &conf,
                &mut section.borrow_mut(),
                option_name.as_ref(),
                value.as_ref(),
            );

            ret as i32
        }

        unsafe extern "C" fn c_write_cb(
            pointer: *const c_void,
            _data: *mut c_void,
            config: *mut t_config_file,
            _section_name: *const c_char,
        ) -> c_int {
            let pointers: &mut ConfigSectionPointers =
                { &mut *(pointer as *mut ConfigSectionPointers) };

            let section = pointers
                .section
                .as_ref()
                .expect("Section reference wasn't set up correctly")
                .upgrade()
                .expect("Config has been destroyed but a read callback run");

            let conf = Conf {
                ptr: config,
                weechat_ptr: pointers.weechat_ptr,
            };
            let weechat = Weechat::from_ptr(pointers.weechat_ptr);

            if let Some(ref mut cb) = pointers.write_cb {
                cb.callback(&weechat, &conf, &mut section.borrow_mut())
            }
            WEECHAT_RC_OK
        }

        unsafe extern "C" fn c_write_default_cb(
            pointer: *const c_void,
            _data: *mut c_void,
            config: *mut t_config_file,
            _section_name: *const c_char,
        ) -> c_int {
            let pointers: &mut ConfigSectionPointers =
                { &mut *(pointer as *mut ConfigSectionPointers) };

            let section = pointers
                .section
                .as_ref()
                .expect("Section reference wasn't set up correctly")
                .upgrade()
                .expect("Config has been destroyed but a read callback run");

            let conf = Conf {
                ptr: config,
                weechat_ptr: pointers.weechat_ptr,
            };
            let weechat = Weechat::from_ptr(pointers.weechat_ptr);

            if let Some(ref mut cb) = pointers.write_default_cb {
                cb.callback(&weechat, &conf, &mut section.borrow_mut())
            }
            WEECHAT_RC_OK
        }

        let weechat = Weechat::from_ptr(self.inner.weechat_ptr);

        let new_section = weechat.get().config_new_section.unwrap();

        let name = LossyCString::new(&section_settings.name);

        let (c_read_cb, read_cb) = match section_settings.read_callback {
            Some(cb) => (Some(c_read_cb as SectionReadCbT), Some(cb)),
            None => (None, None),
        };

        let (c_write_cb, write_cb) = match section_settings.write_callback {
            Some(cb) => (Some(c_write_cb as SectionWriteCbT), Some(cb)),
            None => (None, None),
        };

        let (c_write_default_cb, write_default_cb) = match section_settings.write_default_callback {
            Some(cb) => (Some(c_write_default_cb as SectionWriteCbT), Some(cb)),
            None => (None, None),
        };

        let section_data = Box::new(ConfigSectionPointers {
            read_cb,
            write_cb,
            write_default_cb,
            weechat_ptr: self.inner.weechat_ptr,
            section: None,
        });
        let section_data_ptr = Box::leak(section_data);

        let ptr = unsafe {
            new_section(
                self.inner.ptr,
                name.as_ptr(),
                0,
                0,
                c_read_cb,
                section_data_ptr as *const _ as *const c_void,
                ptr::null_mut(),
                c_write_cb,
                section_data_ptr as *const _ as *const c_void,
                ptr::null_mut(),
                c_write_default_cb,
                section_data_ptr as *const _ as *const c_void,
                ptr::null_mut(),
                None,
                ptr::null_mut(),
                ptr::null_mut(),
                None,
                ptr::null_mut(),
                ptr::null_mut(),
            )
        };

        if ptr.is_null() {
            unsafe { Box::from_raw(section_data_ptr) };
            return Err(());
        };

        let section = ConfigSection {
            ptr,
            config_ptr: self.inner.ptr,
            weechat_ptr: weechat.ptr,
            section_data: section_data_ptr as *const _ as *const c_void,
            name: section_settings.name.clone(),
            option_pointers: HashMap::new(),
        };

        let section = Rc::new(RefCell::new(section));
        let pointers: &mut ConfigSectionPointers =
            unsafe { &mut *(section_data_ptr as *mut ConfigSectionPointers) };

        pointers.section = Some(Rc::downgrade(&section));

        self.sections.insert(section_settings.name.clone(), section);
        let section = &self.sections[&section_settings.name];

        Ok(SectionHandleMut {
            inner: section.borrow_mut(),
        })
    }

    /// Search the configuration object for a section and borrow it.
    ///
    /// Returns a handle to a section if one is found, None otherwise.
    ///
    /// # Arguments
    ///
    /// * `section_name` - The name of the section that should be retrieved.
    ///
    /// # Panics
    ///
    /// This will panic if it is being called in a section read/write callback
    /// of the same section that is being retrieved or if the section is already
    /// mutably borrowed.
    pub fn search_section(&self, section_name: &str) -> Option<SectionHandle> {
        if !self.sections.contains_key(section_name) {
            None
        } else {
            Some(SectionHandle {
                inner: self.sections[section_name].borrow(),
            })
        }
    }

    /// Search the configuration object for a section and borrow it mutably.
    ///
    /// Returns a handle to a section if one is found, None otherwise.
    ///
    /// # Arguments
    ///
    /// * `section_name` - The name of the section that should be retrieved.
    ///
    /// # Panics
    ///
    /// This will panic if it is being called in a section read/write callback
    /// of the same section that is being retrieved or if the section is already
    /// borrowed.
    pub fn search_section_mut(&mut self, section_name: &str) -> Option<SectionHandleMut> {
        if !self.sections.contains_key(section_name) {
            None
        } else {
            Some(SectionHandleMut {
                inner: self.sections[section_name].borrow_mut(),
            })
        }
    }
}

impl Conf {
    /// Write the section header to the configuration file.
    pub fn write_section(&self, section_name: &str) {
        self.write(section_name, None)
    }

    /// Write a line to the configuration file.
    ///
    /// # Arguments
    ///
    /// * `key` - The key of the option that will be written. Can be a
    ///     section name.
    ///
    /// * `value` - The value of the option that will be written.
    pub fn write_line(&self, key: &str, value: &str) {
        self.write(key, Some(value))
    }

    fn write(&self, key: &str, value: Option<&str>) {
        let weechat = Weechat::from_ptr(self.weechat_ptr);
        let write_line = weechat.get().config_write_line.unwrap();

        let option_name = LossyCString::new(key);

        let c_value = match value {
            Some(v) => LossyCString::new(v).as_ptr(),
            None => ptr::null(),
        };

        unsafe {
            write_line(self.ptr, option_name.as_ptr(), c_value);
        }
    }

    /// Write a line in a configuration file with option and its value.
    ///
    /// # Arguments
    ///
    /// * `option` - The option that will be written to the configuration file.
    pub fn write_option<'a, O: AsRef<dyn BaseConfigOption + 'a>>(&self, option: O) {
        let weechat = Weechat::from_ptr(self.weechat_ptr);
        let write_option = weechat.get().config_write_option.unwrap();

        unsafe {
            write_option(self.ptr, option.as_ref().get_ptr());
        }
    }
}
