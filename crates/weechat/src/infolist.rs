//! Infolists can be used to share information between scripts and plugins.
//!
//! The list of available infolists can be found in the Weechat plugin API
//! reference.
//!
//! # Example
//!
//! # Examples
//! ```no_run
//! # use weechat::Weechat;
//! # let weechat = unsafe { weechat::Weechat::weechat() };
//! let infolist = weechat.get_infolist("hook", Some("infolist")).unwrap();
//!
//! for item in infolist {
//!     for variable in &item {
//!         Weechat::print(&format!("{:?}", variable));
//!     }
//! }
//! ```

use std::{
    borrow::Cow,
    cell::Cell,
    collections::{
        hash_map::{IntoIter as IterHashmap, Keys},
        HashMap,
    },
    ffi::CStr,
    fmt::Debug,
    marker::PhantomData,
    ptr,
    rc::Rc,
    time::{Duration, SystemTime},
};

use weechat_sys::{t_gui_buffer, t_infolist};

use crate::{
    buffer::{Buffer, InnerBuffer, InnerBuffers},
    LossyCString, Weechat,
};

/// An infolist is a list of items.
///
/// Each item contains one or more variables.
pub struct Infolist<'a> {
    ptr: *mut t_infolist,
    infolist_name: String,
    weechat: &'a Weechat,
}

/// The type of an infolist variable.
#[derive(Eq, Hash, Debug, PartialEq, Clone)]
#[allow(missing_docs)]
pub enum InfolistType {
    Integer,
    String,
    Time,
    Buffer,
}

impl From<&str> for InfolistType {
    fn from(value: &str) -> Self {
        match value {
            "i" => InfolistType::Integer,
            "s" => InfolistType::String,
            "t" => InfolistType::Time,
            "p" => InfolistType::Buffer,
            v => panic!("Got unexpected value {}", v),
        }
    }
}

/// An item of the infolist.
///
/// Each infolist item may contain multiple values. It essentially acts as a
/// hashmap.
pub struct InfolistItem<'a> {
    ptr: *mut t_infolist,
    weechat: Weechat,
    fields: HashMap<String, InfolistType>,
    infolist: PhantomData<&'a Infolist<'a>>,
}

impl<'a> Debug for InfolistItem<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_map().entries(self.fields.iter()).finish()
    }
}

impl<'a> InfolistItem<'a> {
    fn integer(&self, name: &str) -> i32 {
        let name = LossyCString::new(name);

        let infolist_integer = self.weechat.get().infolist_integer.unwrap();

        unsafe { infolist_integer(self.ptr, name.as_ptr()) }
    }

    fn string(&'a self, name: &str) -> Option<Cow<str>> {
        let name = LossyCString::new(name);

        let infolist_string = self.weechat.get().infolist_string.unwrap();

        unsafe {
            let ptr = infolist_string(self.ptr, name.as_ptr());
            if ptr.is_null() {
                None
            } else {
                Some(CStr::from_ptr(ptr).to_string_lossy())
            }
        }
    }

    fn buffer(&self, name: &str) -> Option<Buffer> {
        let name = LossyCString::new(name);

        let infolist_pointer = self.weechat.get().infolist_pointer.unwrap();

        let ptr = unsafe { infolist_pointer(self.ptr, name.as_ptr()) as *mut t_gui_buffer };

        if ptr.is_null() {
            return None;
        }

        Some(Buffer {
            inner: InnerBuffers::BorrowedBuffer(InnerBuffer {
                weechat: &self.weechat,
                ptr,
                closing: Rc::new(Cell::new(false)),
            }),
        })
    }

    fn time(&self, name: &str) -> Option<SystemTime> {
        let name = LossyCString::new(name);

        let infolist_time = self.weechat.get().infolist_time.unwrap();

        let time = unsafe { infolist_time(self.ptr, name.as_ptr()) };

        let unix = SystemTime::UNIX_EPOCH;
        let duration = Duration::from_secs(time as u64);

        unix.checked_add(duration)
    }

    /// Get a variable from the current infolist item.
    ///
    /// # Arguments
    ///
    /// * `key` - The name of the variable that should be fetched.
    pub fn get(&self, key: &str) -> Option<InfolistVariable> {
        let infolist_type = self.fields.get(key)?;

        let variable = match infolist_type {
            InfolistType::Integer => InfolistVariable::Integer(self.integer(key)),
            InfolistType::String => InfolistVariable::String(self.string(key)?),
            InfolistType::Time => InfolistVariable::Time(self.time(key)?),
            InfolistType::Buffer => InfolistVariable::Buffer(self.buffer(key)?),
        };

        Some(variable)
    }

    /// Get the list of infolist variables that this item has.
    pub fn keys(&self) -> Keys<'_, String, InfolistType> {
        self.fields.keys()
    }

    /// An iterator visiting all variables in an infolist item.
    /// The iterator element type a tuple of a string containing the variable
    /// name and the variable itself.
    ///
    /// # Examples
    /// ```no_run
    /// # use weechat::Weechat;
    /// # use weechat::infolist::InfolistVariable;
    /// # let weechat = unsafe { weechat::Weechat::weechat() };
    /// let infolist = weechat.get_infolist("buffer", None).unwrap();
    ///
    /// for item in infolist {
    ///     for variable in &item {
    ///         Weechat::print(&format!("{:?}", variable));
    ///     }
    /// }
    /// ```
    pub fn iter(&'a self) -> Iter<'a> {
        Iter {
            keys: self.fields.clone().into_iter(),
            item: &self,
        }
    }
}

/// An iterator over the entries of a `InfolistItem`.
///
/// This `struct` is created by the [`iter`] method on [`InfolistItem`]. See its
/// documentation for more.
///
/// [`iter`]: struct.InfolistItem.html#method.iter
/// [`InfolistItem`]: struct.InfolistItem.html
pub struct Iter<'a> {
    item: &'a InfolistItem<'a>,
    keys: IterHashmap<String, InfolistType>,
}

impl<'a> Iterator for Iter<'a> {
    type Item = (String, InfolistVariable<'a>);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let (name, _) = self.keys.next()?;
            let variable = self.item.get(&name);

            if let Some(variable) = variable {
                return Some((name, variable));
            }
        }
    }
}

impl<'a> IntoIterator for &'a InfolistItem<'a> {
    type Item = (String, InfolistVariable<'a>);
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// A variable that was fetched out of the infolist item.
#[derive(Debug, PartialEq, PartialOrd, Eq, Ord)]
pub enum InfolistVariable<'a> {
    /// Represents an infolist integer variable.
    Integer(i32),
    /// Represents an infolist string variable.
    String(Cow<'a, str>),
    /// Represents an infolist time-based variable.
    Time(SystemTime),
    /// Represents an infolist GUI buffer variable.
    Buffer(Buffer<'a>),
}

impl<'a> Infolist<'a> {
    fn is_pointer_buffer(infolist_name: &str, variable_name: &str) -> bool {
        matches!(
            (infolist_name, variable_name),
            ("logger_buffer", "buffer")
                | ("buffer", "pointer")
                | ("buflist", "buffer")
                | ("irc_server", "buffer")
                | ("hotlist", "buffer_pointer")
                | ("window", "buffer")
        )
    }

    fn get_fields(&self) -> HashMap<String, InfolistType> {
        let infolist_fields = self.weechat.get().infolist_fields.unwrap();
        let mut fields: HashMap<String, InfolistType> = HashMap::new();

        let fields_string = unsafe {
            let ptr = infolist_fields(self.ptr);
            CStr::from_ptr(ptr).to_string_lossy()
        };

        for field in fields_string.split(',') {
            let split: Vec<&str> = field.split(':').collect();

            let infolist_type = split[0];
            let name = split[1];

            // Skip the buffer, we can't safely expose them
            // without knowing the size of the buffer. (Note the buffer here
            // isn't a GUI buffer but a vector like thing.
            if infolist_type == "b" {
                continue;
            }

            let field = if infolist_type == "p" {
                if Infolist::is_pointer_buffer(&self.infolist_name, name) {
                    InfolistType::Buffer
                } else {
                    continue;
                }
            } else {
                InfolistType::from(infolist_type)
            };

            fields.insert(name.to_owned(), field);
        }

        fields
    }
}

impl<'a> Drop for Infolist<'a> {
    fn drop(&mut self) {
        let infolist_free = self.weechat.get().infolist_free.unwrap();

        unsafe { infolist_free(self.ptr) }
    }
}

impl Weechat {
    /// Get the infolist with the given name.
    ///
    /// # Arguments
    ///
    /// * `infolist_name` - The name of the infolist to fetch, valid values for
    /// this can be found in the Weechat documentation.
    ///
    /// * `arguments` - Arguments that should be passed to Weechat while
    /// fetching the infolist, the format of this will depend on the infolist
    /// that is being fetched. A list of infolists and their accompanying
    /// arguments can be found in the Weechat documentation.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use weechat::infolist::InfolistVariable;
    /// # let weechat = unsafe { weechat::Weechat::weechat() };
    /// let infolist = weechat.get_infolist("logger_buffer", None).unwrap();
    ///
    /// for item in infolist {
    ///     let info_buffer = if let Some(buffer) = item.get("buffer") {
    ///         buffer
    ///     } else {
    ///         continue;
    ///     };
    ///
    ///     if let InfolistVariable::Buffer(info_buffer) = info_buffer {
    ///         info_buffer.print("Hello world");
    ///     }
    /// }
    /// ```
    pub fn get_infolist(
        &self,
        infolist_name: &str,
        arguments: Option<&str>,
    ) -> Result<Infolist, ()> {
        let infolist_get = self.get().infolist_get.unwrap();

        let name = LossyCString::new(infolist_name);
        let arguments = if let Some(args) = arguments {
            Some(LossyCString::new(args))
        } else {
            None
        };

        let infolist_ptr = unsafe {
            infolist_get(
                self.ptr,
                name.as_ptr(),
                ptr::null_mut(),
                arguments.map_or(ptr::null_mut(), |a| a.as_ptr()),
            )
        };

        if infolist_ptr.is_null() {
            Err(())
        } else {
            Ok(Infolist {
                ptr: infolist_ptr,
                infolist_name: infolist_name.to_owned(),
                weechat: &self,
            })
        }
    }
}

impl<'a> Iterator for Infolist<'a> {
    type Item = InfolistItem<'a>;

    fn next(&mut self) -> Option<InfolistItem<'a>> {
        let infolist_next = self.weechat.get().infolist_next.unwrap();

        let ret = unsafe { infolist_next(self.ptr) };

        if ret == 1 {
            let fields = self.get_fields();

            Some(InfolistItem {
                ptr: self.ptr,
                weechat: Weechat::from_ptr(self.weechat.ptr),
                fields,
                infolist: PhantomData,
            })
        } else {
            None
        }
    }
}
