//! `Dstr::replace` does no heap work when nothing matches (#1296).
//!
//! Counting allocations on the calling thread is deterministic, unlike a
//! wall-clock bound: the old replace built a full copy of the string before
//! learning there was no match, which is what made `dstr/safe_printf` 6.6x
//! to 58x slower than C (one replace per `$n` placeholder).

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use obs_util::dstr::Dstr;

struct Counting;

thread_local! {
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
}

// SAFETY: forwards every call to the system allocator unchanged.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _ = ALLOCS.try_with(|n| n.set(n.get() + 1));
        // SAFETY: same contract as the caller's.
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: same contract as the caller's.
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let _ = ALLOCS.try_with(|n| n.set(n.get() + 1));
        // SAFETY: same contract as the caller's.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

fn allocs_during(f: impl FnOnce()) -> usize {
    let before = ALLOCS.with(Cell::get);
    f();
    ALLOCS.with(Cell::get) - before
}

#[test]
fn replace_without_match_does_not_allocate() {
    let mut d = Dstr::new();
    d.copy("obs ".repeat(1024).as_bytes());
    let before = d.as_bytes().to_vec();
    assert_eq!(allocs_during(|| d.replace(b"$1", b"value")), 0);
    assert_eq!(d.as_bytes(), &before[..]);
}

#[test]
fn safe_printf_unused_placeholders_only_copy_the_format() {
    let mut d = Dstr::new();
    let fmt = b"no placeholders here".to_vec();
    // One allocation for the format copy; the `$1`/`$2` scans add none.
    assert_eq!(
        allocs_during(|| d.safe_printf(Some(&fmt), [Some(b"x"), Some(b"y"), None, None])),
        1
    );
}

#[test]
fn replace_with_matches_allocates_at_most_once() {
    let mut d = Dstr::new();
    d.copy("obs ".repeat(1024).as_bytes());
    assert!(allocs_during(|| d.replace(b"obs", b"Rust OBS")) <= 1);
    assert_eq!(d.as_bytes(), "Rust OBS ".repeat(1024).as_bytes());
}

// ---------------------------------------------------------------------------
// The C ABI shims edit the C buffer in place (#1297): an append or a
// safe_printf makes no Rust-side heap allocation (the C buffer itself grows
// through `brealloc`, which this counter does not see), so the existing
// string is never copied into an owned model and back.

mod ffi_in_place {
    use core::ffi::c_char;
    use core::ptr;

    use obs_util::ffi::darray::bfree;
    use obs_util::ffi::dstr::{self as r, Strref, dstr};

    use super::allocs_during;

    struct Owned(dstr);

    impl Owned {
        fn with_spare(len: usize, spare: usize) -> Self {
            let mut d = Owned(dstr {
                array: ptr::null_mut(),
                len: 0,
                capacity: 0,
            });
            let seed = "obs ".repeat(len / 4) + "\0";
            // SAFETY: valid empty dstr and NUL-terminated seed.
            unsafe { r::dstr_copy(&mut d.0, seed.as_ptr() as *const c_char) };
            let pad = vec![b'x'; spare];
            // SAFETY: as above; grows the buffer, then trims back to `len`.
            unsafe {
                r::dstr_ncat(&mut d.0, pad.as_ptr() as *const c_char, spare);
                r::dstr_remove(&mut d.0, len, spare);
            }
            d
        }

        fn bytes(&self) -> &[u8] {
            // SAFETY: a non-null array holds `len` bytes plus a NUL.
            unsafe { core::slice::from_raw_parts(self.0.array as *const u8, self.0.len + 1) }
        }
    }

    impl Drop for Owned {
        fn drop(&mut self) {
            // SAFETY: the array came from bmalloc/brealloc (or is null).
            unsafe { bfree(self.0.array as *mut _) };
        }
    }

    #[test]
    fn ncat_onto_4096_with_spare_capacity_does_not_allocate() {
        let mut d = Owned::with_spare(4096, 64);
        let cap = d.0.capacity;
        assert!(cap > 4096 + 8);
        // SAFETY: valid dstr; 8 readable bytes.
        let n = allocs_during(|| unsafe { r::dstr_ncat(&mut d.0, c" capture".as_ptr(), 8) });
        assert_eq!(n, 0);
        assert_eq!(d.0.len, 4104);
        assert_eq!(d.0.capacity, cap);
        assert_eq!(&d.bytes()[4096..], b" capture\0");
    }

    #[test]
    fn appends_that_grow_do_not_allocate_rust_side() {
        let mut d = Owned::with_spare(128, 0);
        let mut s = Owned::with_spare(16, 0);
        let sr = Strref {
            array: c"strref".as_ptr(),
            len: 6,
        };
        // SAFETY: valid dstrs and strref.
        let n = allocs_during(|| unsafe {
            r::dstr_ncat(&mut d.0, c" capture".as_ptr(), 8);
            r::dstr_cat_dstr(&mut d.0, &s.0);
            r::dstr_cat_strref(&mut d.0, &sr);
            r::dstr_ncat_dstr(&mut d.0, &s.0, 4);
            let p = &raw mut s.0;
            r::dstr_cat_dstr(p, p);
        });
        assert_eq!(n, 0);
        assert_eq!(d.0.len, 128 + 8 + 16 + 6 + 4);
        assert_eq!(s.0.len, 32);
    }

    #[test]
    fn safe_printf_does_not_allocate_rust_side() {
        let mut d = Owned::with_spare(4096, 0);
        let val = "obs ".repeat(1024) + "\0";
        // SAFETY: valid dstr; NUL-terminated strings.
        let n = allocs_during(|| unsafe {
            r::dstr_safe_printf(
                &mut d.0,
                c"$1: $2".as_ptr(),
                val.as_ptr() as *const c_char,
                c"42".as_ptr(),
                ptr::null(),
                ptr::null(),
            )
        });
        assert_eq!(n, 0);
        assert_eq!(d.0.len, 4096 + 4);
        assert!(d.bytes().ends_with(b"obs : 42\0"));
    }
}

// ---------------------------------------------------------------------------
// #1296: a format with placeholders is substituted in one pass, so the result
// costs one allocation instead of a copy plus one rebuild per placeholder.

mod safe_printf_one_pass {
    use core::ptr;

    use obs_util::dstr::Dstr;
    use obs_util::ffi::darray::bfree;
    use obs_util::ffi::dstr::{self as r, dstr};

    use super::allocs_during;

    #[test]
    fn safe_printf_with_placeholders_allocates_once() {
        let mut d = Dstr::new();
        let fmt = b"$1: $2".to_vec();
        assert_eq!(
            allocs_during(|| d.safe_printf(Some(&fmt), [Some(b"x"), Some(b"y"), None, None])),
            1
        );
        assert_eq!(d.as_bytes(), b"x: y");
    }

    #[test]
    fn ffi_safe_printf_matches_the_sequential_passes() {
        for value in [&b"obs obs"[..], &b""[..], &b"$1"[..]] {
            let v1 = std::ffi::CString::new(value).unwrap();
            let mut d = dstr {
                array: ptr::null_mut(),
                len: 0,
                capacity: 0,
            };
            // SAFETY: a valid empty dstr, and NUL-terminated arguments that do
            // not point into its buffer.
            unsafe {
                r::dstr_safe_printf(
                    &mut d,
                    c"[$1|$2]".as_ptr(),
                    v1.as_ptr(),
                    c"42".as_ptr(),
                    ptr::null(),
                    ptr::null(),
                )
            };
            // SAFETY: a non-null array holds `len` bytes plus a NUL.
            let got = unsafe { core::slice::from_raw_parts(d.array as *const u8, d.len) }.to_vec();
            // The value holds a `$`, so C's four passes run: `$1` -> `$1` is a
            // no-op and the later pass still substitutes `$2`.
            let expect = match value {
                b"$1" => b"[$1|42]".to_vec(),
                other => [b"[".as_slice(), other, b"|42]"].concat(),
            };
            assert_eq!(got, expect, "value {:?}", String::from_utf8_lossy(value));
            // SAFETY: the array came from bmalloc/brealloc.
            unsafe { bfree(d.array as *mut _) };
        }
    }
}
