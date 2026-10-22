#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| pswd_core::fuzz::decode_payload(data));
