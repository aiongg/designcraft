use designcraft_geom::Rect;

use super::*;
use crate::build::NewDocument;

fn doc() -> Document {
    Document::new(&NewDocument { pages: 2, ..Default::default() })
}

#[test]
fn threading_moves_frames_and_text() {
    let mut d = doc();
    let lid = d.default_layer();
    let (a, sa) = d.add_text_frame(SpreadRef::Doc(0), Rect::new(10.0, 10.0, 100.0, 100.0), lid, "first", ParaFormat::default()).unwrap();
    let (b, sb) = d.add_text_frame(SpreadRef::Doc(0), Rect::new(110.0, 10.0, 200.0, 100.0), lid, "second", ParaFormat::default()).unwrap();
    d.thread(a, b).unwrap();
    d.check().unwrap();
    assert!(d.story(sb).is_none());
    let st = d.story(sa).unwrap();
    assert_eq!(st.text, "first\nsecond");
    assert_eq!(st.frames, vec![a, b]);
    assert_eq!(d.next_frame(a), Some(b));
    assert_eq!(d.prev_frame(b), Some(a));
    d.unthread_after(a).unwrap();
    d.check().unwrap();
    assert_eq!(d.story(sa).unwrap().frames, vec![a]);
    assert_ne!(d.item(b).unwrap().text_frame().unwrap().story, sa);
}

#[test]
fn thread_to_empty_frame_converts_it() {
    let mut d = doc();
    let lid = d.default_layer();
    let (a, sa) = d.add_text_frame(SpreadRef::Doc(0), Rect::new(10.0, 10.0, 100.0, 100.0), lid, "x", ParaFormat::default()).unwrap();
    let id = ItemId(d.alloc());
    let it = Item::new(id, lid, Shape::Rectangle, designcraft_geom::shapes::rectangle(Rect::new(0.0, 200.0, 50.0, 300.0)));
    d.insert_item(SpreadRef::Doc(0), it, None).unwrap();
    d.thread(a, id).unwrap();
    assert!(d.item(id).unwrap().is_text_frame());
    assert_eq!(d.story(sa).unwrap().frames, vec![a, id]);
    d.check().unwrap();
    assert!(d.thread(a, a).is_err());
}

#[test]
fn threading_merges_tables_notes_and_endnotes() {
    let mut d = doc();
    let lid = d.default_layer();
    let (a, sa) = d.add_text_frame(SpreadRef::Doc(0), Rect::new(10.0, 10.0, 100.0, 100.0), lid, "first", ParaFormat::default()).unwrap();
    let (b, sb) = d.add_text_frame(SpreadRef::Doc(0), Rect::new(110.0, 10.0, 200.0, 100.0), lid, "second", ParaFormat::default()).unwrap();
    for (sid, note) in [(sa, "Note A"), (sb, "Note B")] {
        let st = d.story_mut(sid).unwrap();
        st.insert_note(3, note, ParaFormat::default());
        // Both stories hold a table with id 1.
        let n = st.len();
        st.insert_table(n, crate::table::Table::new(1, 2, 2, 0, 0, 80.0));
    }
    let st = d.story_mut(sb).unwrap();
    st.insert_endnote(2, "End B", ParaFormat::default());
    st.insert(1, &crate::NOTE_MARK.to_string());
    d.thread(a, b).unwrap();
    d.check().unwrap();
    let st = d.story(sa).unwrap();
    assert!(d.story(sb).is_none());
    assert_eq!(st.tables.len(), 2);
    assert_eq!(st.notes.len(), 2);
    assert_eq!(st.notes[1].text.text, "Note B");
    assert_eq!(st.endnotes.len(), 1);
    assert_eq!(st.endnotes[0].text.text, "End B");
    assert_eq!(st.editorial.len(), 1);
}

#[test]
fn empty_frame_threaded_before_a_story_joins_it_at_the_front() {
    let mut d = doc();
    let lid = d.default_layer();
    let (a, sa) = d.add_text_frame(SpreadRef::Doc(0), Rect::new(10.0, 10.0, 100.0, 100.0), lid, "", ParaFormat::default()).unwrap();
    let (b, sb) = d.add_text_frame(SpreadRef::Doc(0), Rect::new(110.0, 10.0, 200.0, 100.0), lid, "Hello", ParaFormat::default()).unwrap();
    d.thread_keeping_story(a, b).unwrap();
    d.check().unwrap();
    assert!(d.story(sa).is_none());
    let st = d.story(sb).unwrap();
    assert_eq!(st.frames, vec![a, b]);
    assert_eq!(st.text, "Hello");
}

#[test]
fn empty_story_takes_the_threaded_text_without_a_blank_paragraph() {
    let mut d = doc();
    let lid = d.default_layer();
    let (a, _) = d.add_text_frame(SpreadRef::Doc(0), Rect::new(10.0, 10.0, 100.0, 100.0), lid, "", ParaFormat::default()).unwrap();
    let (a2, _) = d.add_text_frame(SpreadRef::Doc(0), Rect::new(10.0, 110.0, 100.0, 200.0), lid, "", ParaFormat::default()).unwrap();
    d.thread(a, a2).unwrap();
    let (b, _) = d.add_text_frame(SpreadRef::Doc(0), Rect::new(110.0, 10.0, 200.0, 100.0), lid, "Hello", ParaFormat::default()).unwrap();
    d.thread(a2, b).unwrap();
    d.check().unwrap();
    let st = d.story(d.item(a).unwrap().text_frame().unwrap().story).unwrap();
    assert_eq!(st.frames, vec![a, a2, b]);
    assert_eq!(st.text, "Hello");
}

#[test]
fn threading_from_an_empty_frame_makes_it_a_text_frame() {
    let mut d = doc();
    let lid = d.default_layer();
    let id = ItemId(d.alloc());
    let it = Item::new(id, lid, Shape::Rectangle, designcraft_geom::shapes::rectangle(Rect::new(0.0, 200.0, 50.0, 300.0)));
    d.insert_item(SpreadRef::Doc(0), it, None).unwrap();
    let (b, _) = d.add_text_frame(SpreadRef::Doc(0), Rect::new(110.0, 10.0, 200.0, 100.0), lid, "Hello", ParaFormat::default()).unwrap();
    d.thread(id, b).unwrap();
    d.check().unwrap();
    let st = d.story(d.item(id).unwrap().text_frame().unwrap().story).unwrap();
    assert_eq!(st.frames, vec![id, b]);
    assert_eq!(st.text, "Hello");
}

#[test]
fn removing_a_frame_from_its_thread_keeps_the_text_in_the_others() {
    let mut d = doc();
    let lid = d.default_layer();
    let (a, sa) = d.add_text_frame(SpreadRef::Doc(0), Rect::new(10.0, 10.0, 100.0, 100.0), lid, "words", ParaFormat::default()).unwrap();
    let (b, _) = d.add_text_frame(SpreadRef::Doc(0), Rect::new(110.0, 10.0, 200.0, 100.0), lid, "", ParaFormat::default()).unwrap();
    let (c, _) = d.add_text_frame(SpreadRef::Doc(0), Rect::new(210.0, 10.0, 300.0, 100.0), lid, "", ParaFormat::default()).unwrap();
    d.thread(a, b).unwrap();
    d.thread(b, c).unwrap();
    d.remove_from_thread(b).unwrap();
    d.check().unwrap();
    assert_eq!(d.story(sa).unwrap().frames, vec![a, c]);
    assert_eq!(d.story(sa).unwrap().text, "words");
    let own = d.item(b).unwrap().text_frame().unwrap().story;
    assert_ne!(own, sa);
    assert!(d.story(own).unwrap().text.is_empty());
    // A frame alone in its story has no thread to leave.
    assert!(d.remove_from_thread(b).is_err());
}

#[test]
fn removing_last_frame_deletes_story() {
    let mut d = doc();
    let lid = d.default_layer();
    let (a, sa) = d.add_text_frame(SpreadRef::Doc(0), Rect::new(10.0, 10.0, 100.0, 100.0), lid, "x", ParaFormat::default()).unwrap();
    d.remove_item(a).unwrap();
    assert!(d.story(sa).is_none());
    d.check().unwrap();
}

#[test]
fn hit_testing_respects_layers() {
    let mut d = doc();
    let lid = d.default_layer();
    let (a, _) = d.add_text_frame(SpreadRef::Doc(0), Rect::new(10.0, 10.0, 100.0, 100.0), lid, "x", ParaFormat::default()).unwrap();
    assert_eq!(d.hit_item(0, designcraft_geom::Point::new(50.0, 50.0), 2.0), Some(a));
    assert_eq!(d.hit_item(0, designcraft_geom::Point::new(300.0, 300.0), 2.0), None);
    d.layer_mut(lid).unwrap().locked = true;
    assert_eq!(d.hit_item(0, designcraft_geom::Point::new(50.0, 50.0), 2.0), None);
}

#[test]
fn page_names_follow_sections() {
    let mut d = Document::new(&NewDocument { pages: 6, ..Default::default() });
    assert_eq!(d.page_name(0), "1");
    d.sections.push(Section {
        start: 2,
        start_number: Some(1),
        style: NumberStyle::LowerRoman,
        prefix: "A-".into(),
        marker: String::new(),
        include_prefix: true,
    });
    assert_eq!(d.page_name(1), "2");
    assert_eq!(d.page_name(2), "A-i");
    assert_eq!(d.page_name(4), "A-iii");
    // A section that continues numbering.
    d.sections.push(Section {
        start: 5,
        start_number: None,
        style: NumberStyle::Arabic,
        prefix: String::new(),
        marker: String::new(),
        include_prefix: false,
    });
    assert_eq!(d.page_name(5), "4");
}

#[test]
fn document_serde_roundtrip() {
    let mut d = doc();
    let lid = d.default_layer();
    d.add_text_frame(SpreadRef::Doc(0), Rect::new(10.0, 10.0, 100.0, 100.0), lid, "héllo\nworld", ParaFormat::default()).unwrap();
    let s = serde_json::to_string(&d).unwrap();
    let back: Document = serde_json::from_str(&s).unwrap();
    assert_eq!(d, back);
    back.check().unwrap();
}

/// A file can set `next_id` to `u64::MAX`; the next allocated id then overflowed (IDML export
/// adds to it too). Such a document is rejected when it is checked on load.
#[test]
fn next_id_near_the_top_is_rejected() {
    let mut d = Document::new(&crate::build::NewDocument::default());
    d.check().unwrap();
    d.next_id = u64::MAX;
    assert!(d.check().is_err());
    d.next_id = crate::MAX_NEXT_ID;
    d.check().unwrap();
}
