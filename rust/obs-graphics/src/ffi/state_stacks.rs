//! C ABI for `libobs/graphics/graphics-state-stacks.c.h`.
//!
//! `graphics.c` calls these through `graphics-state-stacks.h` on the
//! `graphics_subsystem` darrays; they are not `EXPORT`, so the shims stay
//! hidden in libobs (`libobs/cmake/rust-hidden.txt`). [`DarrayStack`] is the
//! core's [`Stack`] over a C darray, grown and shrunk as the inline
//! functions of `util/darray.h` do, on libobs's allocator through the C
//! adapters (see the note in `ffi/image_file.rs`).
//!
//! Intentional differences, where C dereferences null or reads past its
//! darray, and `graphics.c` never gets there: a null darray, index, value
//! or output pointer does nothing (a pop returns false, the reset no
//! steps); an index past the stack reads and changes nothing.

use core::ffi::c_void;
use core::marker::PhantomData;
use core::mem::size_of;
use core::ptr;

use obs_util::ffi::darray::darray;

use crate::ffi::axisang::axisang;
use crate::ffi::matrix4::matrix4;
use crate::ffi::quat::quat;
use crate::ffi::vec3::vec3;
use crate::matrix4::Matrix4;
use crate::state_stacks::{self, BlendReset, BlendState, GsRect, MatrixOp, Stack};

/// A C darray of `T` as a [`Stack`].
pub struct DarrayStack<'a, T> {
    da: &'a mut darray,
    _t: PhantomData<T>,
}

impl<'a, T> DarrayStack<'a, T> {
    /// # Safety
    ///
    /// `da` is a valid darray of `T`, allocated with `bmalloc` (or empty).
    pub unsafe fn new(da: &'a mut darray) -> Self {
        Self {
            da,
            _t: PhantomData,
        }
    }
}

unsafe extern "C" {
    // libobs's allocator through the private C adapters
    // (libobs/graphics/obs-graphics-adapter.c.h), not obs-util's Rust
    // paths: see the note in ffi/image_file.rs.
    #[link_name = "obs_graphics_bmalloc"]
    fn bmalloc(size: usize) -> *mut c_void;
    #[link_name = "obs_graphics_bfree"]
    fn bfree(ptr: *mut c_void);
}

impl<T> DarrayStack<'_, T> {
    /// `darray_ensure_capacity`: the capacity doubles (or becomes
    /// `new_size` from 0, or when doubling is not enough), and the old
    /// `capacity` elements move to the new buffer.
    fn ensure_capacity(&mut self, new_size: usize) {
        if new_size <= self.da.capacity {
            return;
        }
        let doubled = if self.da.capacity == 0 {
            new_size
        } else {
            self.da.capacity.saturating_mul(2)
        };
        let new_cap = doubled.max(new_size);
        // SAFETY: plain allocation; `bmalloc` never returns null (it aborts).
        let p = unsafe { bmalloc(size_of::<T>().saturating_mul(new_cap)) };
        if !self.da.array.is_null() {
            if self.da.capacity != 0 {
                // SAFETY: the old buffer holds `capacity` elements, the new
                // one `new_cap > capacity`; they are distinct allocations.
                unsafe {
                    ptr::copy_nonoverlapping(
                        self.da.array.cast::<u8>(),
                        p.cast::<u8>(),
                        size_of::<T>().saturating_mul(self.da.capacity),
                    );
                }
            }
            // SAFETY: the old buffer came from `bmalloc`.
            unsafe { bfree(self.da.array) };
        }
        self.da.array = p;
        self.da.capacity = new_cap;
    }
}

impl<T: Copy> Stack<T> for DarrayStack<'_, T> {
    fn len(&self) -> usize {
        self.da.num
    }

    fn get(&self, index: usize) -> Option<T> {
        if index >= self.da.num || self.da.array.is_null() {
            return None;
        }
        // SAFETY: `index < num` elements of `T` (the `new` contract);
        // unaligned, so no alignment of the allocation is assumed.
        Some(unsafe { ptr::read_unaligned(self.da.array.cast::<T>().add(index)) })
    }

    fn set(&mut self, index: usize, value: T) {
        if index < self.da.num && !self.da.array.is_null() {
            // SAFETY: as in `get`.
            unsafe { ptr::write_unaligned(self.da.array.cast::<T>().add(index), value) };
        }
    }

    /// `darray_push_back`. With room in the buffer it writes in place, as
    /// the inline C does with a constant size; otherwise it grows first.
    fn push(&mut self, value: T) {
        if self.da.num >= self.da.capacity || self.da.array.is_null() {
            self.ensure_capacity(self.da.num.saturating_add(1));
        }
        // SAFETY: the buffer holds more than `num` elements of `T`.
        unsafe { ptr::write_unaligned(self.da.array.cast::<T>().add(self.da.num), value) };
        self.da.num = self.da.num.wrapping_add(1);
    }

    /// `darray_erase`: the elements after `index` move down. The last
    /// element moves nothing, so only `num` changes.
    fn erase(&mut self, index: usize) {
        if index.checked_add(1) == Some(self.da.num) {
            self.da.num = index;
            return;
        }
        let Some(rest) = self
            .da
            .num
            .checked_sub(index)
            .and_then(|n| n.checked_sub(1))
        else {
            return;
        };
        let base = self.da.array.cast::<T>();
        // SAFETY: elements `index + 1 ..= index + rest` (< num) are in the
        // buffer; `ptr::copy` allows the overlap.
        unsafe { ptr::copy(base.add(index).add(1), base.add(index), rest) };
        self.da.num = self.da.num.wrapping_sub(1);
    }

    /// `darray_pop_back`.
    fn pop(&mut self) {
        if let Some(last) = self.da.num.checked_sub(1) {
            self.erase(last);
        }
    }
}

/// The matrix stack: a darray of `struct matrix4`, read and written as
/// the C type and converted, so the layout of [`Matrix4`] does not matter.
pub struct MatrixStack<'a>(DarrayStack<'a, matrix4>);

impl Stack<Matrix4> for MatrixStack<'_> {
    fn len(&self) -> usize {
        self.0.len()
    }

    fn get(&self, index: usize) -> Option<Matrix4> {
        self.0.get(index).map(Matrix4::from)
    }

    fn set(&mut self, index: usize, value: Matrix4) {
        self.0.set(index, value.into());
    }

    fn push(&mut self, value: Matrix4) {
        self.0.push(value.into());
    }

    fn erase(&mut self, index: usize) {
        self.0.erase(index);
    }

    fn pop(&mut self) {
        self.0.pop();
    }
}

/// The matrix stack behind `stack`.
///
/// # Safety
///
/// `stack` is null or a valid darray of `struct matrix4`.
unsafe fn matrices<'a>(stack: *mut darray) -> Option<MatrixStack<'a>> {
    // SAFETY: the caller's contract.
    unsafe { stack.as_mut().map(|da| MatrixStack(DarrayStack::new(da))) }
}

/// One operation on the top.
///
/// # Safety
///
/// As [`matrices`].
unsafe fn apply(stack: *mut darray, cur: usize, op: MatrixOp) {
    // SAFETY: the caller's contract.
    if let Some(mut s) = unsafe { matrices(stack) } {
        state_stacks::matrix_apply(&mut s, cur, op);
    }
}

/// `gs_matrix_stack_push()`.
///
/// # Safety
///
/// `stack` is null or a valid darray of `struct matrix4`; `cur` is null or
/// valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_matrix_stack_push(stack: *mut darray, cur: *mut usize) {
    // SAFETY: the caller's contract.
    if let (Some(mut s), Some(cur)) = (unsafe { matrices(stack) }, unsafe { cur.as_mut() }) {
        state_stacks::matrix_push(&mut s, cur);
    }
}

/// `gs_matrix_stack_pop()`.
///
/// # Safety
///
/// As [`gs_matrix_stack_push`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_matrix_stack_pop(stack: *mut darray, cur: *mut usize) -> bool {
    // SAFETY: the caller's contract.
    match (unsafe { matrices(stack) }, unsafe { cur.as_mut() }) {
        (Some(mut s), Some(cur)) => state_stacks::matrix_pop(&mut s, cur),
        _ => false,
    }
}

/// `gs_matrix_stack_identity()`.
///
/// # Safety
///
/// `stack` is null or a valid darray of `struct matrix4`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_matrix_stack_identity(stack: *mut darray, cur: usize) {
    // SAFETY: the caller's contract.
    unsafe { apply(stack, cur, MatrixOp::Identity) };
}

/// `gs_matrix_stack_transpose()`.
///
/// # Safety
///
/// As [`gs_matrix_stack_identity`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_matrix_stack_transpose(stack: *mut darray, cur: usize) {
    // SAFETY: the caller's contract.
    unsafe { apply(stack, cur, MatrixOp::Transpose) };
}

/// `gs_matrix_stack_set()`.
///
/// # Safety
///
/// As [`gs_matrix_stack_identity`]; `matrix` is null or readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_matrix_stack_set(
    stack: *mut darray,
    cur: usize,
    matrix: *const matrix4,
) {
    // SAFETY: the caller's contract.
    if let Some(m) = unsafe { matrix.as_ref() } {
        // SAFETY: as above.
        unsafe { apply(stack, cur, MatrixOp::Set(Matrix4::from(*m))) };
    }
}

/// `gs_matrix_stack_get()`.
///
/// # Safety
///
/// As [`gs_matrix_stack_identity`]; `dst` is null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_matrix_stack_get(stack: *const darray, cur: usize, dst: *mut matrix4) {
    // SAFETY: the caller's contract; the stack is only read.
    if let (Some(s), false) = (unsafe { matrices(stack.cast_mut()) }, dst.is_null())
        && let Some(top) = state_stacks::matrix_get(&s, cur)
    {
        // SAFETY: writable (the contract).
        unsafe { ptr::write(dst, top.into()) };
    }
}

/// `gs_matrix_stack_mul()`.
///
/// # Safety
///
/// As [`gs_matrix_stack_set`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_matrix_stack_mul(
    stack: *mut darray,
    cur: usize,
    matrix: *const matrix4,
) {
    // SAFETY: the caller's contract.
    if let Some(m) = unsafe { matrix.as_ref() } {
        // SAFETY: as above.
        unsafe { apply(stack, cur, MatrixOp::Mul(Matrix4::from(*m))) };
    }
}

/// `gs_matrix_stack_rotquat()`.
///
/// # Safety
///
/// As [`gs_matrix_stack_identity`]; `rot` is null or readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_matrix_stack_rotquat(stack: *mut darray, cur: usize, rot: *const quat) {
    // SAFETY: the caller's contract.
    if let Some(q) = unsafe { rot.as_ref() } {
        // SAFETY: as above.
        unsafe { apply(stack, cur, MatrixOp::RotQuat((*q).into())) };
    }
}

/// `gs_matrix_stack_rotaa()`.
///
/// # Safety
///
/// As [`gs_matrix_stack_identity`]; `rot` is null or readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_matrix_stack_rotaa(
    stack: *mut darray,
    cur: usize,
    rot: *const axisang,
) {
    // SAFETY: the caller's contract.
    if let Some(aa) = unsafe { rot.as_ref() } {
        // SAFETY: as above.
        unsafe { apply(stack, cur, MatrixOp::RotAa((*aa).into())) };
    }
}

/// `gs_matrix_stack_translate()`.
///
/// # Safety
///
/// As [`gs_matrix_stack_identity`]; `pos` is null or readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_matrix_stack_translate(
    stack: *mut darray,
    cur: usize,
    pos: *const vec3,
) {
    // SAFETY: the caller's contract.
    if let Some(v) = unsafe { pos.as_ref() } {
        // SAFETY: as above.
        unsafe { apply(stack, cur, MatrixOp::Translate((*v).into())) };
    }
}

/// `gs_matrix_stack_scale()`.
///
/// # Safety
///
/// As [`gs_matrix_stack_identity`]; `scale` is null or readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_matrix_stack_scale(stack: *mut darray, cur: usize, scale: *const vec3) {
    // SAFETY: the caller's contract.
    if let Some(v) = unsafe { scale.as_ref() } {
        // SAFETY: as above.
        unsafe { apply(stack, cur, MatrixOp::Scale((*v).into())) };
    }
}

/// Push `value` on a darray of `T`.
///
/// # Safety
///
/// `stack` is null or a valid darray of `T`; `value` is null or readable.
unsafe fn push<T: Copy>(stack: *mut darray, value: *const T) {
    // SAFETY: the caller's contract.
    if let (Some(da), Some(v)) = (unsafe { stack.as_mut() }, unsafe { value.as_ref() }) {
        // SAFETY: a darray of `T`.
        state_stacks::state_push(&mut unsafe { DarrayStack::<T>::new(da) }, *v);
    }
}

/// Pop a darray of `T` into `out`.
///
/// # Safety
///
/// `stack` is null or a valid darray of `T`; `out` is null or writable.
unsafe fn pop<T: Copy>(stack: *mut darray, out: *mut T) -> bool {
    // SAFETY: the caller's contract.
    let (Some(da), false) = (unsafe { stack.as_mut() }, out.is_null()) else {
        return false;
    };
    // SAFETY: a darray of `T`.
    match state_stacks::state_pop(&mut unsafe { DarrayStack::<T>::new(da) }) {
        Some(v) => {
            // SAFETY: writable (the contract).
            unsafe { ptr::write(out, v) };
            true
        }
        None => false,
    }
}

/// `gs_viewport_stack_push()`.
///
/// # Safety
///
/// `stack` is null or a valid darray of `struct gs_rect`; `rect` is null or
/// readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_viewport_stack_push(stack: *mut darray, rect: *const GsRect) {
    // SAFETY: the caller's contract.
    unsafe { push(stack, rect) };
}

/// `gs_viewport_stack_pop()`.
///
/// # Safety
///
/// `stack` is null or a valid darray of `struct gs_rect`; `rect` is null or
/// writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_viewport_stack_pop(stack: *mut darray, rect: *mut GsRect) -> bool {
    // SAFETY: the caller's contract.
    unsafe { pop(stack, rect) }
}

/// `gs_blend_state_stack_push()`.
///
/// # Safety
///
/// `stack` is null or a valid darray of `struct blend_state`; `state` is
/// null or readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_blend_state_stack_push(stack: *mut darray, state: *const BlendState) {
    // SAFETY: the caller's contract.
    unsafe { push(stack, state) };
}

/// `gs_blend_state_stack_pop()`.
///
/// # Safety
///
/// `stack` is null or a valid darray of `struct blend_state`; `state` is
/// null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_blend_state_stack_pop(
    stack: *mut darray,
    state: *mut BlendState,
) -> bool {
    // SAFETY: the caller's contract.
    unsafe { pop(stack, state) }
}

/// `gs_blend_state_reset_steps()`.
///
/// # Safety
///
/// `cur` is null or readable, a valid `struct blend_state`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_blend_state_reset_steps(cur: *const BlendState) -> BlendReset {
    // SAFETY: the caller's contract.
    unsafe { cur.as_ref() }.map_or_else(BlendReset::default, state_stacks::blend_reset_steps)
}
