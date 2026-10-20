//! PDF/X-1a export, read back from the exported PDF: no transparency left, only process and spot
//! color, overprint and live text kept, the identification; and the checks refusing what X-1a
//! doesn't allow.

use std::collections::HashMap;
use std::sync::Arc;

use designcraft_color::{Color, ColorType, Swatch, SwatchValue};
use designcraft_compose::Cache;
use designcraft_doc::build::NewDocument;
use designcraft_doc::{Content, Document, Fill, Item, ItemId, ParaFormat, SpreadRef};
use designcraft_geom::Rect;
use hayro_syntax::Pdf;
use hayro_syntax::object::{Dict, Name, Object};

use crate::print_tests::output_intent;
use crate::tests::extract_text;
use crate::{PdfOptions, ShadowMask, Standard, check_pdfx1a, export_pdf_with_report};

fn add(d: &mut Document, r: Rect, swatch: &str, edit: impl FnOnce(&mut Item)) -> ItemId {
    let lid = d.default_layer();
    let id = ItemId(d.alloc());
    let mut b = Item::new(id, lid, designcraft_doc::Shape::Rectangle, designcraft_geom::shapes::rectangle(r));
    b.fill = Fill::swatch(swatch);
    edit(&mut b);
    d.insert_item(SpreadRef::Doc(0), b, None).unwrap();
    id
}

/// A spot Gold background with, over it: a box with a soft drop shadow, a 50% cyan box, a
/// knockout group of two 50% boxes, a magenta box that overprints, and a text frame.
fn transparent_doc() -> (Document, PdfOptions) {
    let mut d = Document::new(&NewDocument::default());
    let gold = Color::cmyk(0.0, 0.3, 0.9, 0.1);
    d.swatches.push(Swatch { value: SwatchValue::Color { color: gold, color_type: ColorType::Spot }, ..Swatch::color("Gold", gold) });
    add(&mut d, Rect::new(0.0, 0.0, 612.0, 792.0), "Gold", |_| {});
    let shadowed = add(&mut d, Rect::new(72.0, 72.0, 172.0, 172.0), "C=0 M=100 Y=0 K=0", |b| {
        b.effects.drop_shadow.on = true;
        b.effects.drop_shadow.opacity = 1.0;
    });
    add(&mut d, Rect::new(300.0, 72.0, 400.0, 172.0), "C=100 M=0 Y=0 K=0", |b| b.opacity = 0.5);
    let lid = d.default_layer();
    let child = |d: &mut Document, r: Rect, swatch: &str| {
        let mut b = Item::new(ItemId(d.alloc()), lid, designcraft_doc::Shape::Rectangle, designcraft_geom::shapes::rectangle(r));
        b.fill = Fill::swatch(swatch);
        b.opacity = 0.5;
        Arc::new(b)
    };
    let items = vec![
        child(&mut d, Rect::new(72.0, 300.0, 172.0, 400.0), "C=100 M=0 Y=0 K=0"),
        child(&mut d, Rect::new(122.0, 350.0, 222.0, 450.0), "C=0 M=0 Y=100 K=0"),
    ];
    let mut g =
        Item::new(ItemId(d.alloc()), lid, designcraft_doc::Shape::Group, designcraft_geom::shapes::rectangle(Rect::new(72.0, 300.0, 222.0, 450.0)));
    g.content = Content::Group { items };
    g.knockout = true;
    d.insert_item(SpreadRef::Doc(0), g, None).unwrap();
    add(&mut d, Rect::new(300.0, 300.0, 400.0, 400.0), "C=0 M=100 Y=0 K=0", |b| b.fill.overprint = true);
    d.add_text_frame(SpreadRef::Doc(0), Rect::new(72.0, 500.0, 400.0, 600.0), lid, "Live text", ParaFormat::default()).unwrap();
    // The shadow's coverage: a ramp from 0 to full strength, offset from the box.
    let (w, h) = (64u32, 64u32);
    let coverage: Vec<u8> = (0..h).flat_map(|_| (0..w).map(move |x| (x * 255 / (w - 1)) as u8)).collect();
    let mask = ShadowMask { rect: Rect::new(77.0, 77.0, 182.0, 182.0), width: w, height: h, coverage: Arc::new(coverage) };
    let opts =
        PdfOptions { standard: Standard::PdfX1a, flatten_dpi: 72.0, shadows: Arc::new(HashMap::from([(shadowed, mask)])), ..PdfOptions::default() };
    (d, opts)
}

/// Every color space family named in the file's resources, images and shadings.
fn families(bytes: &[u8]) -> Vec<String> {
    let pdf = Pdf::new(bytes.to_vec()).unwrap();
    let mut out = Vec::new();
    let mut add = |o: Option<Object<'_>>| {
        let name = match o {
            Some(Object::Name(n)) => Some(n.as_str().to_string()),
            Some(Object::Array(a)) => a.iter::<Name<'_>>().next().map(|n| n.as_str().to_string()),
            _ => None,
        };
        if let Some(n) = name
            && !out.contains(&n)
        {
            out.push(n);
        }
    };
    for o in pdf.objects() {
        let dict: Option<Dict<'_>> = match o {
            Object::Dict(d) => Some(d),
            Object::Stream(s) => Some(s.dict().clone()),
            _ => None,
        };
        let Some(dict) = dict else { continue };
        add(dict.get::<Object<'_>>(b"ColorSpace").filter(|o| !matches!(o, Object::Dict(_))));
        if let Some(spaces) = dict.get::<Dict<'_>>(b"ColorSpace") {
            for key in spaces.keys() {
                add(spaces.get::<Object<'_>>(key.as_ref()));
            }
        }
    }
    out
}

/// A drop shadow over a spot, a 50% object and a knockout group come out with no transparency,
/// only process and spot color, the output intent and the PDF/X-1a identification; overprint
/// and live text survive.
#[test]
fn x1a_export_flattens_transparency_into_inks() {
    let (d, opts) = transparent_doc();
    let r = export_pdf_with_report(&d, &Cache::new(), &opts).unwrap();
    let bytes = &r.bytes;
    let text = String::from_utf8_lossy(bytes);
    assert!(bytes.starts_with(b"%PDF-1.4"), "{:?}", text.get(..8));
    // The only finding: the built-in Coated FOGRA39 output intent is an ICC v4 profile.
    let v4 = |i: &String| i.contains("ICC v4");
    let issues = check_pdfx1a(bytes);
    assert!(issues.iter().all(v4), "{issues:?}");
    let profile = output_intent(bytes).unwrap().profile;
    assert_eq!(profile.get(8) == Some(&4), issues.iter().any(v4), "the v4 warning follows the profile");
    assert_eq!(profile.get(8) == Some(&4), r.warnings.iter().any(|w| w.contains("ICC v4")), "{:?}", r.warnings);
    if profile == craft_cms::builtin::COATED_FOGRA39_ICC {
        assert!(r.warnings.iter().any(|w| w.contains("built-in Coated FOGRA39")), "{:?}", r.warnings);
    }
    assert!(text.contains("/GTS_PDFXVersion(PDF/X-1:2003)"));
    assert!(!text.contains("/NChannel"), "no PDF 1.6 NChannel attributes at PDF 1.4");
    assert!(text.contains("/Trapped/False"), "Trapped");
    assert!(output_intent(bytes).is_some(), "output intent");
    for f in families(bytes) {
        assert!(["DeviceCMYK", "DeviceGray", "Separation", "DeviceN", "Indexed", "Pattern"].contains(&f.as_str()), "color space {f}");
    }
    assert!(text.contains("/Gold"), "the spot ink stays a spot");
    for gone in ["/SMask<<", "/SMask <<", "/S/Transparency", "/ca 0.5", "/CA 0.5", "/BM/Multiply"] {
        assert!(!text.contains(gone), "{gone}");
    }
    let pdf = Pdf::new(bytes.to_vec()).unwrap();
    let states: Vec<(bool, bool, i32)> = pdf
        .objects()
        .into_iter()
        .filter_map(|o| match o {
            Object::Dict(d) => Some((d.get::<bool>(b"op")?, d.get::<bool>(b"OP")?, d.get::<i32>(b"OPM")?)),
            _ => None,
        })
        .collect();
    assert!(states.iter().any(|(op, _, opm)| *op && *opm == 1), "overprint survives: {states:?}");
    assert!(extract_text(bytes).concat().contains("Live text"), "{:?}", extract_text(bytes));
}

/// A minimal PDF 1.4 with a CMYK output intent, the X-1a identification and `content` on its
/// page; `entries` go into the page dictionary.
fn tiny_x1a(entries: &str, content: &str) -> Vec<u8> {
    tiny_x1a_with_profile(entries, content, b"")
}

/// [`tiny_x1a`] whose output intent profile holds `profile`.
fn tiny_x1a_with_profile(entries: &str, content: &str, profile: &[u8]) -> Vec<u8> {
    let objects: [Vec<u8>; 6] = [
        b"<</Type/Catalog/Pages 2 0 R/OutputIntents[<</Type/OutputIntent/S/GTS_PDFX/DestOutputProfile 4 0 R>>]>>".to_vec(),
        b"<</Type/Pages/Kids[3 0 R]/Count 1>>".to_vec(),
        format!("<</Type/Page/Parent 2 0 R/MediaBox[0 0 100 100]/TrimBox[0 0 100 100]{entries}/Contents 5 0 R>>").into_bytes(),
        [format!("<</N 4/Length {}>>\nstream\n", profile.len()).as_bytes(), profile, b"\nendstream"].concat(),
        format!("<</Length {}>>\nstream\n{content}\nendstream", content.len()).into_bytes(),
        b"<</GTS_PDFXVersion(PDF/X-1:2003)/Trapped/False>>".to_vec(),
    ];
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (i, o) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend_from_slice(o);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f\r\n", objects.len() + 1).as_bytes());
    for off in offsets {
        out.extend_from_slice(format!("{off:010} 00000 n\r\n").as_bytes());
    }
    out.extend_from_slice(format!("trailer\n<</Size {}/Root 1 0 R/Info 6 0 R>>\nstartxref\n{xref}\n%%EOF\n", objects.len() + 1).as_bytes());
    out
}

/// The X-1a checks refuse soft masks, constant opacity, blend modes, transparency groups, RGB
/// and ICC-based color; process and spot color pass.
#[test]
fn x1a_checks_refuse_transparency_and_rgb() {
    let clean = check_pdfx1a(&tiny_x1a(
        "/Resources<</ColorSpace<</Gold[/Separation/Gold/DeviceCMYK 7 0 R]>>>>",
        "0 0 0 1 k 0 0 10 10 re f 0.5 G /Gold cs 1 scn 0 0 5 5 re f",
    ));
    assert_eq!(clean, Vec::<String>::new());
    let has = |entries: &str, content: &str, what: &str| {
        let issues = check_pdfx1a(&tiny_x1a(entries, content));
        assert!(issues.iter().any(|i| i.contains(what)), "{what}: {issues:?}");
    };
    has("/Resources<<>>", "1 0 0 rg 0 0 10 10 re f", "DeviceRGB");
    has("/Resources<</ColorSpace<</CS0[/ICCBased 4 0 R]>>>>", "/CS0 cs 0 0 0 1 scn 0 0 10 10 re f", "ICC-based");
    has("/Resources<</ExtGState<</G0<</SMask<</S/Luminosity/G 5 0 R>>>>>>>>", "/G0 gs 0 0 0 1 k 0 0 10 10 re f", "soft mask");
    has("/Resources<</ExtGState<</G0<</ca 0.5>>>>>>", "/G0 gs 0 0 0 1 k 0 0 10 10 re f", "opacity");
    has("/Resources<</ExtGState<</G0<</BM/Multiply>>>>>>", "/G0 gs 0 0 0 1 k 0 0 10 10 re f", "/Multiply");
    has("/Resources<<>>/Group<</S/Transparency/CS/DeviceCMYK>>", "0 0 0 1 k", "transparency group");
    has("/Resources<</ColorSpace<</S[/Separation/Gold/DeviceRGB 7 0 R]>>>>", "/S cs 1 scn 0 0 10 10 re f", "alternate space");
    has("/Resources<<>>", "q BI /W 1 /H 1 /CS /RGB /BPC 8 /F /AHx ID ff0000> EI Q", "DeviceRGB");
    has(
        "/Resources<</ColorSpace<</N[/DeviceN[/Cyan/Gold]/DeviceCMYK 7 0 R<</Subtype/NChannel/Colorants<<>>>>]>>>>",
        "/N cs 1 1 scn 0 0 10 10 re f",
        "NChannel",
    );
    // An ICC v4 output intent profile in a PDF 1.4 file is a warning; v2 passes.
    let header = |major: u8| [&[0u8; 8][..], &[major, 0x20, 0, 0], b"prtrCMYK"].concat();
    let v4 = check_pdfx1a(&tiny_x1a_with_profile("/Resources<<>>", "0 0 0 1 k", &header(4)));
    assert!(v4.iter().any(|i| i.starts_with("warning: ") && i.contains("ICC v4")), "{v4:?}");
    assert_eq!(check_pdfx1a(&tiny_x1a_with_profile("/Resources<<>>", "0 0 0 1 k", &header(2))), Vec::<String>::new());
    // A PDF/X-4 file isn't X-1a.
    let x4 = export_pdf_with_report(
        &Document::new(&NewDocument::default()),
        &Cache::new(),
        &PdfOptions { standard: Standard::PdfX4, ..Default::default() },
    )
    .unwrap();
    let issues = check_pdfx1a(&x4.bytes);
    assert!(issues.iter().any(|i| i.contains("1.4")) && issues.iter().any(|i| i.contains("PDF/X-1:2003")), "{issues:?}");
}

#[test]
fn x1a_parses() {
    for s in ["x1a", "PDF/X-1a", "PDF/X-1a:2003", "pdfx1a"] {
        assert_eq!(Standard::parse(s), Some(Standard::PdfX1a), "{s}");
    }
}
