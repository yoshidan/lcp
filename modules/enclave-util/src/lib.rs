#![no_std]
#[macro_use]
extern crate sgx_tstd as std;
extern crate alloc;

use std::ffi::CString;

extern "C" {
    fn ocall_print(msg: *const i8);
}

#[no_mangle]
pub extern "C" fn log(msg: &str) {
    let s = CString::new(msg).expect("Failed to create CString from message");
    unsafe {
        ocall_print(s.as_ptr());
    }
}