//! The `screenshot` tool against a fake [`CaptureBackend`].

use std::cell::RefCell;
use std::io::Read as _;
use std::rc::Rc;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use obs_mcp::server::{Server, ToolRegistry};
use obs_mcp::tools::screenshot::{CaptureBackend, Frame, encode_png, register};
use serde_json::{Value, json};

type Request = (Option<String>, Option<u32>, Option<u32>);

/// Returns a 3x2 frame, or an error for the source `missing`.
#[derive(Default)]
struct Fake {
    requests: RefCell<Vec<Request>>,
    /// Overrides the frame the fake returns.
    frame: RefCell<Option<Frame>>,
}

fn sample() -> Frame {
    let rgba = (0..3 * 2 * 4)
        .map(|i| u8::try_from(i * 7).unwrap())
        .collect();
    Frame {
        width: 3,
        height: 2,
        rgba,
    }
}

impl CaptureBackend for Fake {
    fn capture(
        &self,
        source: Option<&str>,
        width: Option<u32>,
        height: Option<u32>,
    ) -> Result<Frame, String> {
        self.requests
            .borrow_mut()
            .push((source.map(Into::into), width, height));
        if source == Some("missing") {
            return Err("no source named missing".into());
        }
        Ok(self.frame.borrow().clone().unwrap_or_else(sample))
    }
}

fn server(fake: &Rc<Fake>) -> Server {
    let backend: Rc<dyn CaptureBackend> = fake.clone();
    let mut reg = ToolRegistry::new();
    register(&mut reg, &backend);
    Server::with_tools(reg)
}

fn rpc(s: &mut Server, method: &str, params: &Value) -> Value {
    let line = json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params});
    let mut out = Vec::new();
    s.serve(format!("{line}\n").as_bytes(), &mut out).unwrap();
    serde_json::from_slice(&out).unwrap()
}

fn shoot(s: &mut Server, args: &Value) -> Value {
    rpc(
        s,
        "tools/call",
        &json!({"name": "screenshot", "arguments": args}),
    )["result"]
        .clone()
}

/// Splits a PNG into `(type, data)` chunks, checking the signature and CRCs.
fn chunks(png: &[u8]) -> Vec<([u8; 4], Vec<u8>)> {
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    let mut rest = &png[8..];
    let mut out = Vec::new();
    while !rest.is_empty() {
        let len = u32::from_be_bytes(rest[..4].try_into().unwrap()) as usize;
        let kind: [u8; 4] = rest[4..8].try_into().unwrap();
        let data = rest[8..8 + len].to_vec();
        let crc = u32::from_be_bytes(rest[8 + len..12 + len].try_into().unwrap());
        let mut check = flate2::Crc::new();
        check.update(&rest[4..8 + len]);
        assert_eq!(crc, check.sum(), "bad CRC in {kind:?}");
        out.push((kind, data));
        rest = &rest[12 + len..];
    }
    out
}

/// Decodes a PNG made by the encoder back to `(width, height, rgba)`.
fn decode(png: &[u8]) -> (u32, u32, Vec<u8>) {
    let chunks = chunks(png);
    let kinds: Vec<&[u8]> = chunks.iter().map(|(k, _)| k.as_slice()).collect();
    assert_eq!(kinds.first(), Some(&b"IHDR".as_slice()));
    assert_eq!(kinds.last(), Some(&b"IEND".as_slice()));
    let ihdr = &chunks[0].1;
    let width = u32::from_be_bytes(ihdr[0..4].try_into().unwrap());
    let height = u32::from_be_bytes(ihdr[4..8].try_into().unwrap());
    // 8-bit depth, colour type 6 (RGBA), deflate, adaptive filtering, no interlace.
    assert_eq!(&ihdr[8..], [8, 6, 0, 0, 0]);
    let idat: Vec<u8> = chunks
        .iter()
        .filter(|(k, _)| k == b"IDAT")
        .flat_map(|(_, d)| d.clone())
        .collect();
    let mut raw = Vec::new();
    flate2::read::ZlibDecoder::new(idat.as_slice())
        .read_to_end(&mut raw)
        .unwrap();
    let stride = width as usize * 4;
    assert_eq!(raw.len(), (stride + 1) * height as usize);
    let mut rgba = Vec::new();
    for row in raw.chunks(stride + 1) {
        assert_eq!(row[0], 0, "encoder uses filter type 0");
        rgba.extend_from_slice(&row[1..]);
    }
    (width, height, rgba)
}

#[test]
fn lists_screenshot_with_optional_arguments() {
    let mut s = server(&Rc::default());
    let r = rpc(&mut s, "tools/list", &json!({}));
    let tools = r["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0]["name"], "screenshot");
    let schema = &tools[0]["inputSchema"];
    assert_eq!(schema["type"], "object");
    assert_eq!(schema["additionalProperties"], false);
    let props = schema["properties"].as_object().unwrap();
    let mut keys: Vec<&String> = props.keys().collect();
    keys.sort();
    assert_eq!(keys, ["height", "source", "width"]);
    assert!(
        schema.get("required").is_none(),
        "all arguments are optional"
    );
}

#[test]
fn returns_png_image_content() {
    let fake = Rc::new(Fake::default());
    let mut s = server(&fake);
    let result = shoot(&mut s, &json!({}));
    assert_eq!(result["isError"], false);
    let content = result["content"].as_array().unwrap();
    assert_eq!(content.len(), 2, "the image and its size");
    assert_eq!(content[0]["type"], "image");
    assert_eq!(content[0]["mimeType"], "image/png");
    let png = STANDARD
        .decode(content[0]["data"].as_str().unwrap())
        .unwrap();
    let frame = sample();
    assert_eq!(decode(&png), (frame.width, frame.height, frame.rgba));
}

#[test]
fn no_source_captures_the_program_output() {
    let fake = Rc::new(Fake::default());
    let mut s = server(&fake);
    shoot(&mut s, &json!({}));
    assert_eq!(*fake.requests.borrow(), [(None, None, None)]);
}

#[test]
fn source_and_size_reach_the_backend() {
    let fake = Rc::new(Fake::default());
    let mut s = server(&fake);
    shoot(
        &mut s,
        &json!({"source": "Cam", "width": 640, "height": 360}),
    );
    shoot(&mut s, &json!({"width": 320}));
    assert_eq!(
        *fake.requests.borrow(),
        [
            (Some("Cam".to_owned()), Some(640), Some(360)),
            (None, Some(320), None)
        ]
    );
}

#[test]
fn backend_error_becomes_a_tool_error() {
    let mut s = server(&Rc::default());
    let result = shoot(&mut s, &json!({"source": "missing"}));
    assert_eq!(result["isError"], true);
    assert_eq!(result["content"][0]["type"], "text");
    assert!(
        result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("no source named missing")
    );
}

#[test]
fn invalid_arguments_never_reach_the_backend() {
    let fake = Rc::new(Fake::default());
    let mut s = server(&fake);
    for args in [
        json!({"source": ""}),
        json!({"source": 5}),
        json!({"width": 0}),
        json!({"height": -1}),
        json!({"width": "wide"}),
        json!({"width": 1.5}),
        json!({"width": 99_999_999_999_u64}),
    ] {
        let result = shoot(&mut s, &args);
        assert_eq!(result["isError"], true, "{args}");
    }
    assert!(fake.requests.borrow().is_empty());
}

#[test]
fn a_frame_that_does_not_match_its_size_is_an_error() {
    let fake = Rc::new(Fake::default());
    *fake.frame.borrow_mut() = Some(Frame {
        width: 2,
        height: 2,
        rgba: vec![0; 5],
    });
    let mut s = server(&fake);
    assert_eq!(shoot(&mut s, &json!({}))["isError"], true);
}

#[test]
fn encoder_round_trips_pixels() {
    let frame = Frame {
        width: 1,
        height: 1,
        rgba: vec![255, 0, 128, 7],
    };
    let png = encode_png(&frame).unwrap();
    assert_eq!(decode(&png), (1, 1, frame.rgba));
}

#[test]
fn encoder_rejects_empty_and_mismatched_frames() {
    let empty = Frame {
        width: 0,
        height: 4,
        rgba: vec![],
    };
    assert!(encode_png(&empty).is_err());
    let short = Frame {
        width: 2,
        height: 2,
        rgba: vec![0; 15],
    };
    assert!(encode_png(&short).is_err());
}

#[test]
fn a_text_item_reports_the_size_and_source() {
    let mut s = server(&Rc::default());
    let result = shoot(&mut s, &json!({}));
    let text = &result["content"][1];
    assert_eq!(text["type"], "text");
    let info: Value = serde_json::from_str(text["text"].as_str().unwrap()).unwrap();
    assert_eq!(info, json!({"width": 3, "height": 2, "source": null}));

    let result = shoot(&mut s, &json!({"source": "Cam"}));
    let info: Value =
        serde_json::from_str(result["content"][1]["text"].as_str().unwrap()).unwrap();
    assert_eq!(info, json!({"width": 3, "height": 2, "source": "Cam"}));
}

#[test]
fn the_description_documents_the_alpha_channel() {
    let mut s = server(&Rc::default());
    let r = rpc(&mut s, "tools/list", &json!({}));
    let description = r["result"]["tools"][0]["description"].as_str().unwrap();
    assert!(description.contains("alpha"), "{description}");
    assert!(description.contains("transparent"), "{description}");
}
