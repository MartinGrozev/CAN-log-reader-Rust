//! FFI bindings to the mdflib C API wrapper
//!
//! This module provides safe Rust bindings to the C API wrapper
//! that interfaces with the mdflib C++ library.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_void};

#[repr(C)]
pub struct MdfCanFrame {
    pub timestamp_ns: u64,
    pub channel: u8,
    pub can_id: u32,
    pub data: [u8; 64],
    pub data_length: u8,
    pub is_extended: u8,
    pub is_fd: u8,
    pub is_error_frame: u8,
    pub is_remote_frame: u8,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MdfError {
    Ok = 0,
    OpenFailed = 1,
    NotMdfFile = 2,
    ReadFailed = 3,
    NoCanData = 4,
    NullHandle = 5,
    EndOfData = 6,
}

// Opaque handle types
pub type MdfReaderHandle = *mut c_void;
pub type MdfIteratorHandle = *mut c_void;

#[link(name = "mdf_c_api", kind = "static")]
extern "C" {
    pub fn mdf_open(filename: *const c_char, error: *mut MdfError) -> MdfReaderHandle;
    pub fn mdf_close(reader: MdfReaderHandle);
    pub fn mdf_create_can_iterator(
        reader: MdfReaderHandle,
        error: *mut MdfError,
    ) -> MdfIteratorHandle;
    pub fn mdf_iterator_next(iterator: MdfIteratorHandle, frame: *mut MdfCanFrame) -> MdfError;
    pub fn mdf_iterator_free(iterator: MdfIteratorHandle);
    pub fn mdf_get_error_message() -> *const c_char;
}

/// Safe wrapper around mdf_get_error_message
pub fn get_last_error() -> String {
    unsafe {
        let ptr = mdf_get_error_message();
        if ptr.is_null() {
            return String::from("Unknown error");
        }
        CStr::from_ptr(ptr)
            .to_string_lossy()
            .into_owned()
    }
}
