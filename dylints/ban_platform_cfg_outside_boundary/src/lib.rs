#![feature(rustc_private)]

extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_span;

use rustc_errors::DiagDecorator;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::{FileName, RemapPathScopeComponents, Span};
use std::collections::HashSet;

#[derive(Default)]
struct BanPlatformCfgOutsideBoundary {
    scanned_files: HashSet<String>,
}

dylint_linting::impl_pre_expansion_lint! {
    /// ### What it does
    ///
    /// Enforces the obs-rust/obs-studio#323 platform boundary: in every port
    /// crate (`rust/<crate>/src/**`), host-platform selection and native OS
    /// APIs may appear only in `src/platform/` (the crate's single
    /// `cfg_select!` and its per-OS children) and in `src/ffi/` (the C ABI
    /// shims, whose exported symbol set itself differs per OS). Every other
    /// production source denies host-platform `#[cfg]`, `#[cfg_attr]` and
    /// `cfg!()`, and direct `std::os::{windows,unix,..}`, `libc::` and Win32
    /// paths.
    ///
    /// ### Why
    ///
    /// `#![deny(unsafe_code)]` and clippy only see code that compiles for the
    /// target being linted, and the lint lane runs on Linux. Without this
    /// boundary, `#[cfg(windows)]` code in a safe core is never linted. The
    /// check runs pre-expansion on the physical source, so cfg'd-away code
    /// cannot hide, and one Linux lane covers every host.
    ///
    /// Out of scope: `rust/obs-c-oracle` (test-only, links the original C),
    /// `rust/tools`, and integration tests, benches and examples outside
    /// `src/`.
    ///
    /// Adapted from soldr's `dylints/ban_platform_cfg_outside_boundary`.
    pub BAN_PLATFORM_CFG_OUTSIDE_BOUNDARY,
    Deny,
    "keep host-platform selection inside each crate's src/platform and src/ffi",
    BanPlatformCfgOutsideBoundary::default()
}

const SELECTORS: [&str; 10] = [
    "windows",
    "unix",
    "target_os",
    "target_family",
    "target_arch",
    "target_abi",
    "target_env",
    "target_vendor",
    "target_endian",
    "target_pointer_width",
];

impl EarlyLintPass for BanPlatformCfgOutsideBoundary {
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &rustc_ast::ast::Item) {
        let current_file = source_filename(cx, item.span);
        if !in_scope(&current_file) || !self.scanned_files.insert(current_file.clone()) {
            return;
        }
        // Scan the physical source once, rather than only the active AST.
        // This is what makes cfg-elided items and crate-level inner attrs
        // visible to the boundary.
        let source = std::fs::read_to_string(&current_file)
            .or_else(|_| cx.sess().source_map().span_to_snippet(item.span));
        if let Ok(source) = source {
            for invocation in platform_cfg_invocations(&source) {
                emit(cx, item.span, format!("host cfg `{invocation}`"));
            }
            for reference in native_platform_references(&source) {
                emit(
                    cx,
                    item.span,
                    format!("direct native-platform reference `{reference}`"),
                );
            }
        }
    }
}

fn emit(cx: &EarlyContext<'_>, span: Span, detail: String) {
    cx.opt_span_lint(
        BAN_PLATFORM_CFG_OUTSIDE_BOUNDARY,
        Some(span),
        DiagDecorator(move |diag| {
            diag.primary_message(format!(
                "host-platform selection outside the platform boundary: {detail}; \
                 move it into this crate's src/platform/ (or src/ffi/ for C ABI shims), \
                 see docs/rust-port/testing-policy.md"
            ));
        }),
    );
}

/// Production sources the boundary applies to: every `rust/<crate>/src/**`
/// file (including inline unit-test modules), except the boundary itself
/// (`src/platform/`, `src/ffi/`) and the out-of-scope crates.
fn in_scope(filename: &str) -> bool {
    // Resolve `..` first: a `#[path]`-included test reaches here as
    // `rust/<crate>/src/../tests/...`, which is physically under tests/.
    let normalized = resolve_dot_dot(&filename.replace('\\', "/"));
    if normalized.ends_with("ui/allowed_boundary.rs") {
        return false;
    }
    if normalized.starts_with("ui/") || normalized.contains("/ui/") {
        return true;
    }
    let Some(marker) = normalized.rfind("rust/") else {
        return false;
    };
    let relative = &normalized[marker + "rust/".len()..];
    let Some((krate, rest)) = relative.split_once('/') else {
        return false;
    };
    if !rest.starts_with("src/") || !rest.ends_with(".rs") {
        return false;
    }
    if matches!(krate, "obs-c-oracle" | "tools") {
        return false;
    }
    !(rest.starts_with("src/platform/")
        || rest == "src/platform.rs"
        || rest.starts_with("src/ffi/")
        || rest == "src/ffi.rs")
}

fn resolve_dot_dot(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            ".." if parts.last().is_some_and(|last| !last.is_empty() && *last != "..") => {
                parts.pop();
            }
            "." => {}
            _ => parts.push(part),
        }
    }
    parts.join("/")
}

fn platform_cfg_invocations(source: &str) -> Vec<String> {
    let code = code_without_comments_or_strings(source);
    let compact: String = code
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    let mut invocations = Vec::new();
    for start in ["#[cfg(", "#[cfg_attr(", "#![cfg(", "#![cfg_attr(", "cfg!("] {
        for (offset, _) in compact.match_indices(start) {
            let Some(clause) = balanced_invocation(&compact, offset) else {
                continue;
            };
            if SELECTORS.iter().any(|selector| clause.contains(selector)) {
                invocations.push(clause.trim_start_matches("#[").to_owned());
            }
        }
    }
    invocations
}

fn native_platform_references(source: &str) -> Vec<String> {
    let code = code_without_comments_or_strings(source);
    [
        "std::os::windows",
        "std::os::unix",
        "std::os::linux",
        "std::os::macos",
        "windows_sys",
        "windows::Win32",
        "libc::",
        "tokio::net::windows",
        "tokio::net::Unix",
        "interprocess::os::windows",
        "interprocess::os::unix",
    ]
    .into_iter()
    .filter(|marker| code.contains(marker))
    .map(str::to_owned)
    .collect()
}

fn balanced_invocation(source: &str, offset: usize) -> Option<&str> {
    let mut depth = 0_u32;
    let mut saw_open = false;
    for (relative, character) in source[offset..].char_indices() {
        match character {
            '(' => {
                saw_open = true;
                depth += 1;
            }
            ')' if saw_open => {
                depth -= 1;
                if depth == 0 {
                    return Some(&source[offset..offset + relative + 1]);
                }
            }
            _ => {}
        }
    }
    None
}

fn code_without_comments_or_strings(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut output = String::with_capacity(source.len());
    let mut index = 0;
    while index < bytes.len() {
        if let Some((prefix_len, hashes)) = raw_string_prefix(&bytes[index..]) {
            let start = index;
            index += prefix_len;
            while index < bytes.len() {
                let suffix = &bytes[index + 1..];
                if bytes[index] == b'"'
                    && suffix.len() >= hashes
                    && suffix[..hashes].iter().all(|byte| *byte == b'#')
                {
                    index += 1 + hashes;
                    break;
                }
                index += 1;
            }
            mask_range(&mut output, bytes, start, index);
        } else if bytes[index..].starts_with(b"//") {
            while index < bytes.len() && bytes[index] != b'\n' {
                output.push(' ');
                index += 1;
            }
        } else if bytes[index..].starts_with(b"/*") {
            let mut depth = 1_u32;
            output.push_str("  ");
            index += 2;
            while index < bytes.len() && depth > 0 {
                if bytes[index..].starts_with(b"/*") {
                    depth += 1;
                    output.push_str("  ");
                    index += 2;
                } else if bytes[index..].starts_with(b"*/") {
                    depth -= 1;
                    output.push_str("  ");
                    index += 2;
                } else {
                    output.push(if bytes[index] == b'\n' { '\n' } else { ' ' });
                    index += 1;
                }
            }
        } else if bytes[index] == b'"' {
            output.push(' ');
            index += 1;
            while index < bytes.len() {
                let byte = bytes[index];
                output.push(if byte == b'\n' { '\n' } else { ' ' });
                index += 1;
                if byte == b'\\' && index < bytes.len() {
                    output.push(' ');
                    index += 1;
                } else if byte == b'"' {
                    break;
                }
            }
        } else {
            output.push(bytes[index] as char);
            index += 1;
        }
    }
    output
}

fn raw_string_prefix(source: &[u8]) -> Option<(usize, usize)> {
    let mut index = usize::from(source.starts_with(b"br"));
    if source.get(index) != Some(&b'r') {
        return None;
    }
    index += 1;
    let hashes_start = index;
    while source.get(index) == Some(&b'#') {
        index += 1;
    }
    (source.get(index) == Some(&b'"')).then_some((index + 1, index - hashes_start))
}

fn mask_range(output: &mut String, source: &[u8], start: usize, end: usize) {
    for byte in &source[start..end] {
        output.push(if *byte == b'\n' { '\n' } else { ' ' });
    }
}

fn source_filename(cx: &EarlyContext<'_>, span: Span) -> String {
    match cx.sess().source_map().span_to_filename(span) {
        FileName::Real(real_filename) => real_filename
            .local_path()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_else(|| {
                real_filename
                    .path(RemapPathScopeComponents::DIAGNOSTICS)
                    .to_string_lossy()
                    .into_owned()
            }),
        filename => filename
            .display(RemapPathScopeComponents::DIAGNOSTICS)
            .to_string(),
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}

#[test]
fn source_detector_ignores_comments_and_strings() {
    assert!(platform_cfg_invocations(
        r####"fn neutral() {
            let _ = "#[cfg(windows)]";
            let _ = r###"cfg!(target_arch = "x86_64")"###;
            /* cfg!(unix) */
        }"####
    )
    .is_empty());
    assert_eq!(
        platform_cfg_invocations("fn selected() { if cfg!(windows) {} }"),
        vec!["cfg!(windows)"]
    );
    // feature cfgs are allowed: only the host selectors count
    assert!(platform_cfg_invocations("#[cfg(feature = \"tokio-console\")] fn f() {}").is_empty());
    // test-only cfg is still host selection when a host selector appears
    assert_eq!(
        platform_cfg_invocations("#[cfg(all(test, target_os = \"windows\"))] fn f() {}"),
        vec!["cfg(all(test,target_os=))"]
    );
}

#[test]
fn native_platform_references_are_detected_outside_strings() {
    assert_eq!(
        native_platform_references("use std::os::unix::fs::PermissionsExt; libc::getpid();"),
        vec!["std::os::unix", "libc::"]
    );
    assert!(native_platform_references("let text = \"windows_sys\";").is_empty());
}

#[test]
fn scope_covers_safe_cores_only() {
    for path in [
        "/repo/rust/obs-util/src/pipe.rs",
        "/repo/rust/obs-util/src/sub/../pipe.rs",
        "/repo/rust/obs-media-io/src/audio_io.rs",
        "/repo/rust/obs-util/src/lib.rs",
    ] {
        assert!(in_scope(path), "{path}");
    }
    for path in [
        "/repo/rust/obs-util/src/platform/mod.rs",
        "/repo/rust/obs-util/src/platform/windows.rs",
        "/repo/rust/obs-util/src/ffi/pipe.rs",
        "/repo/rust/obs-c-oracle/src/lib.rs",
        "/repo/rust/obs-util/tests/pipe.rs",
        // A `#[path]`-included test file: physically under tests/.
        "/repo/rust/obs-util/src/../tests/threading/threading.rs",
        "/repo/rust/obs-util/src/../tests/../../obs-c-oracle/tests/oracle/private_symbols.rs",
        "/repo/rust/tools/sanitize/src/main.rs",
    ] {
        assert!(!in_scope(path), "{path}");
    }
}
