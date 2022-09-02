use std::{borrow::Cow, collections::HashMap, ffi::c_void, marker::PhantomData};

use crate::{buffer::Buffer, Weechat};
use weechat_sys::{t_hdata, t_weechat_plugin};

/// An iterator that steps over the lines of the buffer.
pub struct BufferLines<'a> {
    pub(crate) weechat_ptr: *mut t_weechat_plugin,
    pub(crate) first_line: *mut c_void,
    pub(crate) last_line: *mut c_void,
    pub(crate) buffer: PhantomData<&'a Buffer<'a>>,
    pub(crate) done: bool,
}

impl<'a> Iterator for BufferLines<'a> {
    type Item = BufferLine<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            None
        } else {
            let weechat = Weechat::from_ptr(self.weechat_ptr);

            let line_hdata = unsafe { weechat.hdata_get("line") };

            let line_data_pointer =
                unsafe { weechat.hdata_pointer(line_hdata, self.first_line, "data") };

            if line_data_pointer.is_null() {
                return None;
            }

            if self.first_line == self.last_line {
                self.done = true;
            }

            self.first_line = unsafe { weechat.hdata_move(line_hdata, self.first_line, 1) };

            Some(BufferLine {
                weechat,
                line_data_pointer,
                buffer: PhantomData,
            })
        }
    }
}

impl<'a> DoubleEndedIterator for BufferLines<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.done {
            None
        } else {
            let weechat = Weechat::from_ptr(self.weechat_ptr);

            let line_hdata = unsafe { weechat.hdata_get("line") };

            let line_data_pointer =
                unsafe { weechat.hdata_pointer(line_hdata, self.last_line, "data") };

            if line_data_pointer.is_null() {
                return None;
            }

            if self.last_line == self.first_line {
                self.done = true;
            }

            self.last_line = unsafe { weechat.hdata_move(line_hdata, self.last_line, -1) };

            Some(BufferLine {
                weechat,
                line_data_pointer,
                buffer: PhantomData,
            })
        }
    }
}

/// Struct that can be used to update multiple line fields at once.
#[allow(missing_docs)]
#[derive(Debug, Clone, Default)]
pub struct LineData<'a> {
    pub prefix: Option<&'a str>,
    pub message: Option<&'a str>,
    pub date: Option<i64>,
    pub date_printed: Option<i64>,
    pub tags: Option<&'a [&'a str]>,
}

/// The buffer line, makes it possible to modify the printed message and other
/// line data.
pub struct BufferLine<'a> {
    weechat: Weechat,
    line_data_pointer: *mut c_void,
    buffer: PhantomData<&'a Buffer<'a>>,
}

impl<'a> BufferLine<'a> {
    fn hdata(&self) -> *mut t_hdata {
        unsafe { self.weechat.hdata_get("line_data") }
    }

    fn update_line(&self, hashmap: HashMap<&str, &str>) {
        unsafe {
            self.weechat
                .hdata_update(self.hdata(), self.line_data_pointer, hashmap);
        }
    }

    /// Get the prefix of the line, everything left of the message separator
    /// (usually `|`) is considered the prefix.
    pub fn prefix(&self) -> Cow<str> {
        unsafe {
            self.weechat
                .hdata_string(self.hdata(), self.line_data_pointer, "prefix")
        }
    }

    /// Set the prefix to the given new value.
    ///
    /// # Arguments
    ///
    /// * `new_prefix` - The new prefix that should be set on the line.
    pub fn set_prefix(&self, new_prefix: &str) {
        let mut hashmap = HashMap::new();
        hashmap.insert("prefix", new_prefix);
        self.update_line(hashmap);
    }

    /// Get the message of the line.
    pub fn message(&self) -> Cow<str> {
        unsafe {
            self.weechat
                .hdata_string(self.hdata(), self.line_data_pointer, "message")
        }
    }

    /// Set the message to the given new value.
    ///
    /// # Arguments
    ///
    /// * `new_value` - The new message that should be set on the line.
    pub fn set_message(&self, new_value: &str) {
        let mut hashmap = HashMap::new();

        hashmap.insert("message", new_value);
        self.update_line(hashmap);
    }

    /// Get the date of the line.
    pub fn date(&self) -> i64 {
        unsafe {
            self.weechat
                .hdata_time(self.hdata(), self.line_data_pointer, "date")
        }
    }

    /// Set the date to the given new value.
    ///
    /// # Arguments
    ///
    /// * `new_value` - The new date that should be set on the line.
    pub fn set_date(&self, new_value: i64) {
        let mut hashmap = HashMap::new();
        let date = new_value.to_string();
        hashmap.insert("date", date.as_ref());
        self.update_line(hashmap);
    }

    /// Get the date the line was printed.
    pub fn date_printed(&self) -> i64 {
        unsafe {
            self.weechat
                .hdata_time(self.hdata(), self.line_data_pointer, "date_printed")
        }
    }

    /// Set the date the line was printed to the given new value.
    ///
    /// # Arguments
    ///
    /// * `new_value` - The new date that should be set on the line.
    pub fn set_date_printed(&self, new_value: &str) {
        let mut hashmap = HashMap::new();
        let date = new_value.to_string();
        hashmap.insert("date_printed", date.as_ref());
        self.update_line(hashmap);
    }

    /// Is the line highlighted.
    pub fn highlighted(&self) -> bool {
        unsafe {
            self.weechat
                .hdata_char(self.hdata(), self.line_data_pointer, "highlight")
                != 0
        }
    }

    /// Get the list of tags of the line.
    pub fn tags(&self) -> Vec<Cow<str>> {
        unsafe {
            let count = self.weechat.hdata_var_array_size(
                self.hdata(),
                self.line_data_pointer,
                "tags_array",
            );

            let mut tags = Vec::with_capacity(count as usize);

            for i in 0..count {
                let tag = self.weechat.hdata_string(
                    self.hdata(),
                    self.line_data_pointer,
                    &format!("{}|tags_array", i),
                );
                tags.push(tag);
            }

            tags
        }
    }

    /// Set the tags of the line to the new value.
    ///
    /// # Arguments
    ///
    /// * `new_value` - The new tags that should be set on the line.
    pub fn set_tags(&self, new_value: &[&str]) {
        let mut hashmap = HashMap::new();
        let tags = new_value.join(",");
        hashmap.insert("tags_array", tags.as_ref());
        self.update_line(hashmap);
    }

    /// Update multiple fields of the line at once.
    ///
    /// # Arguments
    /// * `data` - `LineData` that contains new values that should be set on the
    ///     line.
    ///
    /// # Example
    /// ```no_run
    /// # use weechat::Weechat;
    /// # use weechat::buffer::{BufferBuilder, LineData};
    /// # let buffer_handle = BufferBuilder::new("test")
    /// #    .build()
    /// #    .unwrap();
    /// # let buffer = buffer_handle.upgrade().unwrap();
    /// # let mut lines = buffer.lines();
    /// # let line = lines.next().unwrap();
    ///
    /// let new_line = LineData {
    ///     message: Some("Hello world"),
    ///     tags: Some(&["First", "Second tag"]),
    ///     .. Default::default()
    /// };
    ///
    /// line.update(new_line)
    /// ```
    pub fn update(&self, data: LineData<'a>) {
        let mut hashmap = HashMap::new();

        let tags = data.tags.map(|t| t.join(","));
        let date = data.date.map(|d| d.to_string());
        let date_printed = data.date_printed.map(|d| d.to_string());

        if let Some(message) = data.message {
            hashmap.insert("message", message);
        }

        if let Some(prefix) = data.prefix {
            hashmap.insert("prefix", prefix);
        }

        if let Some(t) = tags.as_ref() {
            hashmap.insert("tags_array", t);
        }

        if let Some(d) = date.as_ref() {
            hashmap.insert("date", d);
        }

        if let Some(d) = date_printed.as_ref() {
            hashmap.insert("date_printed", d);
        }

        self.update_line(hashmap);
    }
}
