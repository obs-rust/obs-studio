//! The dialog text is localized (obs-rust/obs-studio#945): the report names
//! a locale key and positional arguments for its summary and causes, and
//! the en-US strings must say exactly what the English JSON says.

use std::collections::HashMap;
use std::path::Path;

use obs_gpu_status::report::Report;
use obs_gpu_status::*;

fn en_us() -> HashMap<String, String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../frontend/data/locale/en-US.ini");
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter_map(|l| {
            let (k, v) = l.split_once('=')?;
            Some((k.to_owned(), v.trim_matches('"').replace("\\\"", "\"")))
        })
        .collect()
}

/// QTStr(key).arg(...) in the frontend: %1, %2, ... replaced in order.
fn render(template: &str, args: &[String]) -> String {
    let mut out = template.to_owned();
    for (i, a) in args.iter().enumerate().rev() {
        out = out.replace(&format!("%{}", i + 1), a);
    }
    out
}

fn reports() -> Vec<Report> {
    let gpu = GpuDevice {
        vendor: Vendor::Nvidia,
        device_id: Some("2504".into()),
        name: Some("NVIDIA GeForce RTX 3060".into()),
        render_node: true,
    };
    let slow = FrameStats {
        median_frame_ns: 95_000_000,
        interval_ns: 33_333_333,
    };
    vec![
        Report::new(
            Some("NVIDIA GeForce RTX 3060/PCIe/SSE2"),
            vec![gpu.clone()],
            None,
        ),
        Report::new(
            Some("Mesa llvmpipe (LLVM 21.1.8, 256 bits)"),
            vec![gpu.clone()],
            None,
        ),
        Report::new(Some("Mesa llvmpipe (LLVM 21.1.8, 256 bits)"), vec![], None),
        Report::new(None, vec![gpu.clone()], None),
        Report::new(Some("NVIDIA GeForce RTX 3060"), vec![gpu], Some(slow)),
    ]
}

#[test]
fn every_summary_and_cause_has_an_en_us_string_matching_the_english_text() {
    let strings = en_us();
    for r in reports() {
        let template = strings
            .get(r.summary_key)
            .unwrap_or_else(|| panic!("missing {}", r.summary_key));
        assert_eq!(
            render(template, &r.summary_args),
            r.summary,
            "{}",
            r.summary_key
        );
        assert_eq!(r.cause_keys.len(), r.causes.len());
        for (key, english) in r.cause_keys.iter().zip(&r.causes) {
            let template = strings.get(*key).unwrap_or_else(|| panic!("missing {key}"));
            assert_eq!(template, english, "{key}");
        }
    }
}

#[test]
fn every_readout_text_has_an_en_us_string_matching_the_english_text() {
    let strings = en_us();
    let sample = Some(usage::UsageSample {
        percent: 37,
        source: usage::Source::Nvml,
    });
    let mut readouts = Vec::new();
    for r in reports() {
        readouts.push(r.clone().readout());
        readouts.push(r.clone().with_usage_stale(true).readout());
        readouts.push(r.with_usage(sample).readout());
    }
    // The labels: percent, n/a, CPU rendering; the details: no source, late.
    let keys: std::collections::BTreeSet<_> = readouts
        .iter()
        .flat_map(|o| std::iter::once(&o.label).chain(&o.details))
        .map(|t| t.key)
        .collect();
    assert_eq!(keys.len(), 5, "{keys:?}");
    for o in &readouts {
        for t in std::iter::once(&o.label).chain(&o.details) {
            let template = strings
                .get(t.key)
                .unwrap_or_else(|| panic!("missing {}", t.key));
            assert_eq!(render(template, &t.args), t.english, "{}", t.key);
        }
    }
}

#[test]
fn json_carries_the_keys_and_args() {
    let r = Report::new(Some("Mesa llvmpipe"), vec![], None);
    let j = r.to_json();
    assert!(
        j.contains("\"summaryKey\":\"Basic.StatusBar.GpuStatus.Summary.NoGpu\""),
        "{j}"
    );
    assert!(j.contains("\"summaryArgs\":[\"Mesa llvmpipe\"]"), "{j}");
    assert!(j.contains("\"causeKeys\":[\"Basic.StatusBar.GpuStatus.Cause.Driver\",\"Basic.StatusBar.GpuStatus.Cause.RenderNode\"]"), "{j}");
}
