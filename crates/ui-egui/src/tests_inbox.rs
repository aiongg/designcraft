//! Files delivered through the inbox (the browser's asynchronous picker and drops) are handled
//! as the request that asked for them says, in the document it was made from.

use designcraft_engine::Session;
use serde_json::{Value, json};

use crate::{DesignApp, FilePurpose, FileRequest, Inbox, InboxFile, Services};

fn app() -> (DesignApp, Inbox) {
    let inbox = Inbox::default();
    let app = DesignApp::new(Session::new(), Services { inbox: Some(inbox.clone()), ..Default::default() });
    (app, inbox)
}

fn new_doc(app: &mut DesignApp, title: &str) -> u64 {
    app.run("file.new", json!({"title": title})).unwrap();
    app.session.active().unwrap().uid
}

fn deliver(inbox: &Inbox, purpose: FilePurpose, doc: Option<u64>, name: &str, bytes: Vec<u8>) {
    inbox.lock().unwrap().push(InboxFile { request: FileRequest { purpose, doc }, name: name.into(), bytes });
}

/// Top-level objects of the open document titled `title`.
fn items(app: &DesignApp, title: &str) -> usize {
    let st = app.session.documents().iter().find(|d| d.doc.title == title).unwrap();
    st.doc.spreads.iter().map(|sp| sp.items.len()).sum()
}

fn active_title(app: &DesignApp) -> String {
    app.session.active().unwrap().doc.title.clone()
}

fn png() -> Vec<u8> {
    designcraft_render::Rendered { width: 8, height: 8, pixels: vec![255; 8 * 8 * 4] }.to_png()
}

/// A one-page layout with one frame, as `.designcraft` bytes and as IDML bytes.
fn layout() -> (Vec<u8>, Vec<u8>) {
    let mut s = Session::new();
    s.execute("file.new", &json!({"title": "B"})).unwrap();
    s.execute("frame.create", &json!({"rect": [100, 100, 200, 200]})).unwrap();
    let native = s.execute("file.serialize", &json!({})).unwrap();
    let idml = s.execute("file.exportIdml", &json!({})).unwrap();
    let bytes = |v: &Value| designcraft_engine::cmd::base64_decode(v["base64"].as_str().unwrap());
    (bytes(&native), bytes(&idml))
}

#[test]
fn place_of_a_layout_imports_its_page_objects() {
    let (native, idml) = layout();
    for (name, bytes) in [("B.designcraft", native), ("B.idml", idml)] {
        let (mut app, inbox) = app();
        let a = new_doc(&mut app, "A");
        deliver(&inbox, FilePurpose::Place, Some(a), name, bytes);
        app.drain_inbox();
        assert_eq!(app.session.documents().len(), 1, "{name}: placing doesn't open a document");
        assert_eq!(active_title(&app), "A");
        assert_eq!(items(&app, "A"), 1, "{name}: B's frame is placed into A");
    }
}

#[test]
fn place_goes_into_the_document_it_was_chosen_in() {
    let (mut app, inbox) = app();
    let a = new_doc(&mut app, "A");
    new_doc(&mut app, "B");
    deliver(&inbox, FilePurpose::Place, Some(a), "red.png", png());
    app.drain_inbox();
    assert_eq!(items(&app, "A"), 1, "the image lands in A");
    assert_eq!(items(&app, "B"), 0, "B is unchanged");
    assert_eq!(active_title(&app), "A", "A is active again");

    // The document closed before the file arrived: nothing is placed anywhere.
    deliver(&inbox, FilePurpose::Place, Some(a), "red.png", png());
    let index = app.session.documents().iter().position(|d| d.uid == a).unwrap();
    app.run("file.close", json!({"index": index})).unwrap();
    app.drain_inbox();
    assert_eq!(app.session.documents().len(), 1);
    assert_eq!(items(&app, "B"), 0, "B is still unchanged");
    assert!(app.ui.status.contains("red.png") && app.ui.status.contains("closed"), "{}", app.ui.status);
}

#[test]
fn place_of_a_multipage_pdf_asks_which_page() {
    let mut src = Session::new();
    src.execute("file.new", &json!({"pages": 3})).unwrap();
    let pdf = designcraft_engine::cmd::base64_decode(src.execute("file.exportPdf", &json!({})).unwrap()["base64"].as_str().unwrap());
    let (mut app, inbox) = app();
    let a = new_doc(&mut app, "A");
    deliver(&inbox, FilePurpose::Place, Some(a), "three.pdf", pdf);
    app.drain_inbox();
    let d = app.ui.dialog.as_ref().expect("the PDF import dialog opens");
    assert_eq!(d.id, "pdfImport");
    assert_eq!(d.fields["pages"], 3);
    assert_eq!(items(&app, "A"), 0, "nothing is placed before a page is chosen");

    app.ui.dialog.as_mut().unwrap().fields.insert("page".into(), json!("2"));
    crate::dialogs::confirm(&mut app).unwrap();
    assert_eq!(items(&app, "A"), 1);
    let doc = &app.session.active().unwrap().doc;
    let asset = doc.assets.values().next().unwrap();
    assert_eq!(asset.page, 1, "page 2 (zero-based 1) is placed");
}

#[test]
fn dropped_files_open_documents_and_place_the_rest() {
    let (native, _) = layout();
    let (mut app, inbox) = app();
    new_doc(&mut app, "A");
    deliver(&inbox, FilePurpose::Drop, None, "red.png", png());
    app.drain_inbox();
    assert_eq!(items(&app, "A"), 1, "an image is placed into the active document");
    deliver(&inbox, FilePurpose::Drop, None, "B.designcraft", native);
    app.drain_inbox();
    assert_eq!(app.session.documents().len(), 2, "a document opens");
    assert_eq!(active_title(&app), "B");
}

#[test]
fn open_opens_a_document() {
    let (native, _) = layout();
    let (mut app, inbox) = app();
    let a = new_doc(&mut app, "A");
    deliver(&inbox, FilePurpose::Open, Some(a), "B.designcraft", native);
    app.drain_inbox();
    assert_eq!(app.session.documents().len(), 2);
    assert_eq!(active_title(&app), "B");
    assert_eq!(items(&app, "A"), 0);
}

#[test]
fn swatches_load_into_the_document_they_were_chosen_in() {
    let mut src = Session::new();
    src.execute("file.new", &json!({})).unwrap();
    src.execute("swatch.create", &json!({"name": "Brand Teal", "color": "#108080"})).unwrap();
    let ase = src.execute("swatch.save", &json!({"names": ["Brand Teal"]})).unwrap();
    let ase = designcraft_engine::cmd::base64_decode(ase["base64"].as_str().unwrap());
    let (mut app, inbox) = app();
    let a = new_doc(&mut app, "A");
    new_doc(&mut app, "B");
    deliver(&inbox, FilePurpose::Swatches, Some(a), "brand.ase", ase);
    app.drain_inbox();
    let has = |title: &str| app.session.documents().iter().find(|d| d.doc.title == title).unwrap().doc.swatch("Brand Teal").is_some();
    assert!(has("A") && !has("B"));
}
