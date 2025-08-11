#![no_std]
#[macro_use]
extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

extern "C" {
    fn ocall_print(msg: *const i8);
}

#[no_mangle]
pub extern "C" fn log(msg: String) {
    let mut buf: Vec<u8> = msg.as_bytes().to_vec();
    buf.push(0); // NUL終端追加

    unsafe {
        ocall_print(buf.as_ptr() as *const i8);
    }
}