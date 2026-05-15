use std::ffi::c_char;

/// Generate a PDF from JSON report data.
///
/// Returns 0 on success, negative on error.
/// Caller must free the result buffer with `openaec_result_free`.
#[unsafe(no_mangle)]
pub extern "C" fn openaec_generate(
    _json_ptr: *const u8,
    _json_len: usize,
    _out_ptr: *mut *mut u8,
    _out_len: *mut usize,
) -> i32 {
    -1 // Not yet implemented
}

/// Free a PDF result buffer returned by `openaec_generate`.
#[unsafe(no_mangle)]
pub extern "C" fn openaec_result_free(_ptr: *mut u8, _len: usize) {
    // Not yet implemented
}

/// Validate JSON report data against the schema.
///
/// Returns 0 if valid, negative on error.
#[unsafe(no_mangle)]
pub extern "C" fn openaec_validate(
    _json_ptr: *const u8,
    _json_len: usize,
) -> i32 {
    -1 // Not yet implemented
}

/// Get the last error message as a null-terminated C string.
#[unsafe(no_mangle)]
pub extern "C" fn openaec_last_error() -> *const c_char {
    std::ptr::null()
}
