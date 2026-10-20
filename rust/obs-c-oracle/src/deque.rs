use core::ffi::c_void;

/// Independent declaration of `struct deque`. Intentionally not shared
/// with `obs-util`.
#[repr(C)]
#[derive(Debug)]
pub struct OracleDeque {
    pub data: *mut c_void,
    pub size: usize,
    pub start_pos: usize,
    pub end_pos: usize,
    pub capacity: usize,
}

unsafe extern "C" {
    pub fn oracle_deque_init(dq: *mut OracleDeque);
    pub fn oracle_deque_free(dq: *mut OracleDeque);
    pub fn oracle_deque_reserve(dq: *mut OracleDeque, capacity: usize);
    pub fn oracle_deque_upsize(dq: *mut OracleDeque, size: usize);
    pub fn oracle_deque_place(
        dq: *mut OracleDeque,
        position: usize,
        data: *const c_void,
        size: usize,
    );
    pub fn oracle_deque_push_back(dq: *mut OracleDeque, data: *const c_void, size: usize);
    pub fn oracle_deque_push_front(dq: *mut OracleDeque, data: *const c_void, size: usize);
    pub fn oracle_deque_push_back_zero(dq: *mut OracleDeque, size: usize);
    pub fn oracle_deque_push_front_zero(dq: *mut OracleDeque, size: usize);
    pub fn oracle_deque_peek_front(dq: *mut OracleDeque, data: *mut c_void, size: usize);
    pub fn oracle_deque_peek_back(dq: *mut OracleDeque, data: *mut c_void, size: usize);
    pub fn oracle_deque_pop_front(dq: *mut OracleDeque, data: *mut c_void, size: usize);
    pub fn oracle_deque_pop_back(dq: *mut OracleDeque, data: *mut c_void, size: usize);
    pub fn oracle_deque_data(dq: *mut OracleDeque, idx: usize) -> *mut c_void;

    pub fn oracle_deque_size() -> usize;
    pub fn oracle_deque_align() -> usize;
    pub fn oracle_deque_offset_data() -> usize;
    pub fn oracle_deque_offset_size() -> usize;
    pub fn oracle_deque_offset_start_pos() -> usize;
    pub fn oracle_deque_offset_end_pos() -> usize;
    pub fn oracle_deque_offset_capacity() -> usize;
}
