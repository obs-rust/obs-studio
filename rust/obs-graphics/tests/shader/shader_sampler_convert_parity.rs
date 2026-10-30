//! Tiers 2/3: exact real-header layout and original-C parity. No intentional
//! differences for defined inputs. NULL handles, invalid/mismatched arrays,
//! aliased output and empty BorderColor strings are outside the C contract.
//! Native strtol supplies target-specific long overflow/conversion and errno.
//! Keys and filter/address values stay ASCII: the original astrcmpi passes
//! plain char to toupper, which is undefined for negative non-EOF bytes.
//! Numeric and ignored values still cover arbitrary non-NUL bytes.
use obs_c_oracle::shader_sampler_convert as c;
use obs_graphics::ffi::shader_sampler_convert as rs;
use obs_util::ffi::darray::darray;
use proptest::prelude::*;
use std::ffi::{CString, c_void};
use std::mem::{align_of, offset_of, size_of};

#[test]
fn layouts_match_real_c_headers() {
    let actual = [
        size_of::<rs::shader_sampler>(),
        align_of::<rs::shader_sampler>(),
        offset_of!(rs::shader_sampler, name),
        offset_of!(rs::shader_sampler, states),
        offset_of!(rs::shader_sampler, values),
        size_of::<rs::gs_sampler_info>(),
        align_of::<rs::gs_sampler_info>(),
        offset_of!(rs::gs_sampler_info, filter),
        offset_of!(rs::gs_sampler_info, address_u),
        offset_of!(rs::gs_sampler_info, address_v),
        offset_of!(rs::gs_sampler_info, address_w),
        offset_of!(rs::gs_sampler_info, max_anisotropy),
        offset_of!(rs::gs_sampler_info, border_color),
    ];
    for (index, value) in actual.into_iter().enumerate() {
        // SAFETY: the probe accepts each index in its 13-element layout table.
        assert_eq!(
            value,
            unsafe { c::oracle_shader_sampler_layout(index) },
            "layout index {index}"
        );
    }
}

unsafe extern "C" fn rust_bridge(sampler: *mut c_void, info: *mut c_void) {
    // SAFETY: the fixture provides the real ABI layouts and live string arrays.
    unsafe { rs::shader_sampler_convert(sampler.cast(), info.cast()) };
}

fn compare(states: &[(Vec<u8>, Vec<u8>)]) {
    let names: Vec<_> = states
        .iter()
        .map(|(name, _)| CString::new(name.clone()).unwrap())
        .collect();
    let values: Vec<_> = states
        .iter()
        .map(|(_, value)| CString::new(value.clone()).unwrap())
        .collect();
    let mut name_ptrs: Vec<_> = names.iter().map(|s| s.as_ptr()).collect();
    let mut value_ptrs: Vec<_> = values.iter().map(|s| s.as_ptr()).collect();
    let mut sampler = rs::shader_sampler {
        name: core::ptr::null_mut(),
        states: darray {
            array: name_ptrs.as_mut_ptr().cast(),
            num: names.len(),
            capacity: names.len(),
        },
        values: darray {
            array: value_ptrs.as_mut_ptr().cast(),
            num: values.len(),
            capacity: values.len(),
        },
    };
    let poison = rs::gs_sampler_info {
        filter: -123,
        address_u: -123,
        address_v: -123,
        address_w: -123,
        max_anisotropy: -123,
        border_color: 0xa5a5a5a5,
    };
    let mut cv = poison;
    let mut rv = poison;
    let sp = (&mut sampler as *mut rs::shader_sampler).cast();
    // SAFETY: both callbacks receive initialized, nonaliasing real layouts and
    // live NUL-terminated strings; generated BorderColor values are nonempty.
    let (ce, re) = unsafe {
        (
            c::oracle_shader_sampler_errno(
                c::oracle_shader_sampler_convert,
                sp,
                (&mut cv as *mut rs::gs_sampler_info).cast(),
            ),
            c::oracle_shader_sampler_errno(
                rust_bridge,
                sp,
                (&mut rv as *mut rs::gs_sampler_info).cast(),
            ),
        )
    };
    assert_eq!(cv, rv, "states {states:?}");
    assert_eq!(ce, re, "errno for {states:?}");
    assert!(sampler.name.is_null());
    assert_eq!(sampler.states.array, name_ptrs.as_mut_ptr().cast());
    assert_eq!(sampler.values.array, value_ptrs.as_mut_ptr().cast());
    assert_eq!(
        (sampler.states.num, sampler.states.capacity),
        (names.len(), names.len())
    );
    assert_eq!(
        (sampler.values.num, sampler.values.capacity),
        (values.len(), values.len())
    );
    for (index, (name, value)) in states.iter().enumerate() {
        assert_eq!(name_ptrs[index], names[index].as_ptr());
        assert_eq!(value_ptrs[index], values[index].as_ptr());
        assert_eq!(names[index].as_bytes(), name);
        assert_eq!(values[index].as_bytes(), value);
    }
}

#[test]
fn fixed_numeric_and_duplicate_edges_match() {
    compare(&[]);
    for value in [
        "",
        "-1",
        "0x10",
        " +123tail",
        "2147483648",
        "4294967295",
        "9223372036854775807",
        "9223372036854775808",
        "-9223372036854775809",
        "999999999999999999999999999999999999999",
    ] {
        compare(&[
            (b"MaxAnisotropy".to_vec(), value.as_bytes().to_vec()),
            (b"BorderColor".to_vec(), format!("#{value}").into_bytes()),
        ]);
    }
    compare(&[
        (b"Filter".to_vec(), b"Point".to_vec()),
        (b"FILTER".to_vec(), b"unknown".to_vec()),
        (b"AddressU".to_vec(), b"Wrap".to_vec()),
        (b"ADDRESSU".to_vec(), b"MirrorOnce".to_vec()),
        (b"BorderColor".to_vec(), b"x1234".to_vec()),
    ]);
}
fn bytes() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(1u8..=255, 0..72)
}
fn pair() -> impl Strategy<Value = (Vec<u8>, Vec<u8>)> {
    let names = prop_oneof![
        Just(b"Filter".to_vec()),
        Just(b"AddressU".to_vec()),
        Just(b"AddressV".to_vec()),
        Just(b"AddressW".to_vec()),
        Just(b"MaxAnisotropy".to_vec()),
        Just(b"BorderColor".to_vec()),
        prop::collection::vec(1u8..=127, 0..72)
    ];
    let values = prop_oneof![
        bytes(),
        any::<i64>().prop_map(|n| n.to_string().into_bytes()),
        Just(b"Point".to_vec()),
        Just(b"MIN_MAG_POINT_MIP_LINEAR".to_vec()),
        Just(b"Wrap".to_vec()),
        Just(b"MirrorOnce".to_vec()),
        Just(b"#FFFFFFFFFFFFFFFFFFFFFFFF".to_vec())
    ];
    (names, values, any::<bool>()).prop_map(|(mut name, mut value, lower)| {
        if lower {
            name.make_ascii_lowercase();
        }
        if name.eq_ignore_ascii_case(b"Filter")
            || [b"AddressU".as_slice(), b"AddressV", b"AddressW"]
                .iter()
                .any(|key| name.eq_ignore_ascii_case(key))
        {
            value.retain(u8::is_ascii);
        }
        if name.eq_ignore_ascii_case(b"BorderColor") && value.is_empty() {
            value.push(b'#');
        }
        (name, value)
    })
}
proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(512)))]
    #[test]
    fn arbitrary_states_match(states in prop::collection::vec(pair(),0..40)) { compare(&states); }
}

#[test]
fn null_state_and_address_values_match_c() {
    let mut names = [
        core::ptr::null(),
        c"AddressU".as_ptr(),
        c"AddressV".as_ptr(),
        c"AddressW".as_ptr(),
        c"unknown".as_ptr(),
    ];
    let mut values = [core::ptr::null::<std::ffi::c_char>(); 5];
    let mut sampler = rs::shader_sampler {
        name: core::ptr::null_mut(),
        states: darray {
            array: names.as_mut_ptr().cast(),
            num: 5,
            capacity: 5,
        },
        values: darray {
            array: values.as_mut_ptr().cast(),
            num: 5,
            capacity: 5,
        },
    };
    let mut cv = rs::gs_sampler_info {
        filter: -1,
        address_u: -1,
        address_v: -1,
        address_w: -1,
        max_anisotropy: -1,
        border_color: 1,
    };
    let mut rv = cv;
    // SAFETY: C astrcmpi explicitly accepts NULL state/address strings. Unknown
    // states never inspect their value. No Filter/numeric value is NULL here.
    unsafe {
        c::oracle_shader_sampler_convert(
            (&mut sampler as *mut rs::shader_sampler).cast(),
            (&mut cv as *mut rs::gs_sampler_info).cast(),
        );
        rs::shader_sampler_convert(&mut sampler, &mut rv);
    }
    assert_eq!(cv, rv);
    assert_eq!((rv.address_u, rv.address_v, rv.address_w), (0, 0, 0));
}
