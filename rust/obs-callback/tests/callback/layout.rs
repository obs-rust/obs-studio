//! `struct calldata`, `struct decl_param` and `struct decl_info` match the
//! layout the C compiler gives `libobs/callback/calldata.h` and
//! `libobs/callback/decl.h`.

use core::mem::{align_of, offset_of, size_of};

use core::ffi::c_int;

use obs_c_oracle::calldata as c;
use obs_c_oracle::decl as d;
use obs_callback::ffi::calldata::calldata;
use obs_callback::ffi::decl::{decl_info, decl_param};

#[test]
fn calldata_layout_matches_c_header() {
    // SAFETY: the oracle layout functions take no arguments and return constants.
    unsafe {
        assert_eq!(size_of::<calldata>(), c::oracle_sizeof_calldata());
        assert_eq!(align_of::<calldata>(), c::oracle_alignof_calldata());
        assert_eq!(
            offset_of!(calldata, stack),
            c::oracle_offsetof_calldata_stack()
        );
        assert_eq!(
            offset_of!(calldata, size),
            c::oracle_offsetof_calldata_size()
        );
        assert_eq!(
            offset_of!(calldata, capacity),
            c::oracle_offsetof_calldata_capacity()
        );
        assert_eq!(
            offset_of!(calldata, fixed),
            c::oracle_offsetof_calldata_fixed()
        );
    }
}

#[test]
fn decl_layout_matches_c_header() {
    // SAFETY: the oracle layout functions take no arguments and return constants.
    unsafe {
        assert_eq!(size_of::<c_int>(), d::oracle_call_param_type_size());

        assert_eq!(size_of::<decl_param>(), d::oracle_decl_param_size());
        assert_eq!(align_of::<decl_param>(), d::oracle_decl_param_align());
        assert_eq!(
            offset_of!(decl_param, name),
            d::oracle_decl_param_offset_name()
        );
        assert_eq!(
            offset_of!(decl_param, r#type),
            d::oracle_decl_param_offset_type()
        );
        assert_eq!(
            offset_of!(decl_param, flags),
            d::oracle_decl_param_offset_flags()
        );

        assert_eq!(size_of::<decl_info>(), d::oracle_decl_info_size());
        assert_eq!(align_of::<decl_info>(), d::oracle_decl_info_align());
        assert_eq!(
            offset_of!(decl_info, name),
            d::oracle_decl_info_offset_name()
        );
        assert_eq!(
            offset_of!(decl_info, decl_string),
            d::oracle_decl_info_offset_decl_string()
        );
        assert_eq!(
            offset_of!(decl_info, params),
            d::oracle_decl_info_offset_params()
        );
    }
}
