use std::{
    borrow::Cow,
    collections::HashMap,
    ffi::{c_void, CStr},
    os::raw::c_char,
};
use weechat_sys::t_hdata;

use crate::{LossyCString, Weechat};

impl Weechat {
    pub(crate) unsafe fn hdata_get(&self, name: &str) -> *mut t_hdata {
        let hdata_get = self.get().hdata_get.unwrap();

        let name = LossyCString::new(name);

        hdata_get(self.ptr, name.as_ptr())
    }

    pub(crate) unsafe fn hdata_pointer(
        &self,
        hdata: *mut t_hdata,
        pointer: *mut c_void,
        name: &str,
    ) -> *mut c_void {
        let hdata_pointer = self.get().hdata_pointer.unwrap();
        let name = LossyCString::new(name);

        hdata_pointer(hdata, pointer, name.as_ptr())
    }

    pub(crate) unsafe fn hdata_integer(
        &self,
        hdata: *mut t_hdata,
        pointer: *mut c_void,
        name: &str,
    ) -> i32 {
        let hdata_integer = self.get().hdata_integer.unwrap();
        let name = LossyCString::new(name);

        hdata_integer(hdata, pointer, name.as_ptr())
    }

    pub(crate) unsafe fn hdata_time(
        &self,
        hdata: *mut t_hdata,
        pointer: *mut c_void,
        name: &str,
    ) -> i64 {
        let hdata_time = self.get().hdata_time.unwrap();
        let name = LossyCString::new(name);

        hdata_time(hdata, pointer, name.as_ptr())
    }

    pub(crate) unsafe fn hdata_char(
        &self,
        hdata: *mut t_hdata,
        pointer: *mut c_void,
        name: &str,
    ) -> c_char {
        let hdata_char = self.get().hdata_char.unwrap();
        let name = LossyCString::new(name);

        hdata_char(hdata, pointer, name.as_ptr())
    }

    pub(crate) unsafe fn hdata_var_array_size(
        &self,
        hdata: *mut t_hdata,
        pointer: *mut c_void,
        name: &str,
    ) -> i32 {
        let hdata_get_var_array_size = self.get().hdata_get_var_array_size.unwrap();
        let name = LossyCString::new(name);

        hdata_get_var_array_size(hdata, pointer, name.as_ptr())
    }

    pub(crate) unsafe fn hdata_move(
        &self,
        hdata: *mut t_hdata,
        pointer: *mut c_void,
        offset: i32,
    ) -> *mut c_void {
        let hdata_move = self.get().hdata_move.unwrap();
        hdata_move(hdata, pointer, offset)
    }

    pub(crate) unsafe fn hdata_string(
        &self,
        hdata: *mut t_hdata,
        pointer: *mut c_void,
        name: &str,
    ) -> Cow<str> {
        let hdata_string = self.get().hdata_string.unwrap();
        let name = LossyCString::new(name);

        let string_ptr = hdata_string(hdata, pointer, name.as_ptr());
        CStr::from_ptr(string_ptr).to_string_lossy()
    }

    pub(crate) unsafe fn hdata_update(
        &self,
        hdata: *mut t_hdata,
        pointer: *mut c_void,
        hashmap: HashMap<&str, &str>,
    ) -> i32 {
        let hdata_update = self.get().hdata_update.unwrap();

        let hashtable = self.hashmap_to_weechat(hashmap);
        let ret = hdata_update(hdata, pointer, hashtable);
        self.get().hashtable_free.unwrap()(hashtable);
        ret
    }
}
