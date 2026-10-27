#[cfg(not(target_arch = "wasm32"))]
use super::catalog_faces::FaceKey;
#[cfg(not(target_arch = "wasm32"))]
use super::db::NEXT_SCOPE;
#[cfg(not(target_arch = "wasm32"))]
use super::face::{
    DocumentFonts, FontBytes, FontFace, FontSource, MAX_DOCUMENT_FONT_BYTES, MAX_DOCUMENT_FONT_FILES, MAX_DOCUMENT_FONTS_TOTAL, faces_at, make_face,
};
#[cfg(not(target_arch = "wasm32"))]
use super::fallback::SYSTEM_FALLBACKS;
#[cfg(not(target_arch = "wasm32"))]
use super::names::{aliases, face_names, in_catalog_family, norm, wws_names};
use super::*;
#[cfg(not(target_arch = "wasm32"))]
use craft_font_catalog::{FaceInfo, Source};

impl FontFace {
    /// When the font file was added (created, else last modified): the font menus' Show Recently
    /// Added. `None` for fonts without a file.
    pub fn added(&self) -> Option<std::time::SystemTime> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let meta = std::fs::metadata(self.source.path()?).ok()?;
            meta.created().or_else(|_| meta.modified()).ok()
        }
        #[cfg(target_arch = "wasm32")]
        {
            None
        }
    }
}

/// A font file's length and modification time.
#[cfg(not(target_arch = "wasm32"))]
pub(super) type FileStamp = (u64, Option<std::time::SystemTime>);

#[cfg(not(target_arch = "wasm32"))]
fn file_stamp(path: &std::path::Path) -> Option<FileStamp> {
    std::fs::metadata(path).ok().map(|m| (m.len(), m.modified().ok()))
}

/// What [`FontDb::refresh`] (and [`FontDb::reload_fonts`]) did: the installed faces listed, and
/// the installed font files that changed or were removed since faces were made from them (those
/// faces are let go: the next lookup reads the file as it is now, or finds the font missing).
#[derive(Debug, Default, PartialEq)]
pub struct ReloadedFonts {
    pub cataloged: usize,
    pub changed: Vec<std::path::PathBuf>,
    /// The families removed or changed (a style installed or removed, a file replaced), and those
    /// of the faces let go: what was composed with them is composed again.
    pub families: Vec<String>,
    /// The catalog no longer knows which families changed since the last listing: compose
    /// everything again.
    pub full: bool,
}

/// One installed face as lookups see it: the catalog's record, and what the listing read of its
/// file besides.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug)]
pub(super) struct CatalogEntry {
    pub(super) family: String,
    pub(super) style: String,
    pub(super) wws: Option<(String, String)>,
    pub(super) info: FaceInfo,
    /// The font menu group and native family name.
    pub(super) group: FontGroup,
    pub(super) native: Option<String>,
    pub(super) aliases: Vec<String>,
    pub(super) postscript: Option<String>,
}

/// Wakes the app when a rescan changed the installed fonts ([`FontDb::set_change_waker`]).
static WAKER: std::sync::OnceLock<Box<dyn Fn() + Send + Sync>> = std::sync::OnceLock::new();

/// The catalog's faces as lookups see them, and the catalog generation they were listed at.
#[cfg(not(target_arch = "wasm32"))]
pub(super) type Listing = (u64, Arc<[CatalogEntry]>);

/// What the listing reads of one face in a font file that the catalog doesn't give: its font menu
/// group and native family name, its WWS names, and its family's names in every language.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, PartialEq)]
pub(super) struct ScannedFace {
    /// The face's number in its file.
    pub(super) index: u32,
    wws: Option<(String, String)>,
    group: FontGroup,
    native: Option<String>,
    pub(super) aliases: Vec<String>,
}

/// The tables glyph outlines are drawn from.
#[cfg(not(target_arch = "wasm32"))]
const OUTLINE_TABLES: [&[u8; 4]; 4] = [b"glyf", b"CFF ", b"CFF2", b"VARC"];

/// The font menu group and names of every face in the font file at `path`, reading only its table
/// directories and the tables that name and classify it (`name`, `OS/2`, `meta`; `cmap` only for a
/// font that declares no language or code page): a listing opens hundreds of font files, many of
/// them megabytes long. A variable font is read for its default instance. Faces without glyph
/// outlines (none of [`OUTLINE_TABLES`]) are left out.
#[cfg(not(target_arch = "wasm32"))]
pub(super) fn file_face_names(path: &std::path::Path) -> Vec<ScannedFace> {
    use std::io::{Read, Seek, SeekFrom};
    /// Caps on what a (possibly damaged) file can make the scan read.
    const MAX_FACES: u32 = 256;
    const MAX_NAME_TABLE: u32 = 1 << 20;
    const MAX_SMALL_TABLE: u32 = 64 << 10;
    const MAX_CMAP: u32 = 4 << 20;
    let Ok(mut file) = std::fs::File::open(path) else { return vec![] };
    let mut read_at = |offset: u64, len: usize| -> Option<Vec<u8>> {
        let mut buf = vec![0; len];
        file.seek(SeekFrom::Start(offset)).ok()?;
        file.read_exact(&mut buf).ok()?;
        Some(buf)
    };
    let be32 = |b: &[u8], at: usize| b.get(at..at + 4).and_then(|s| s.try_into().ok()).map(u32::from_be_bytes);
    let Some(head) = read_at(0, 12) else { return vec![] };
    // A collection lists where each face's table directory starts.
    let starts: Vec<u32> = if head.starts_with(b"ttcf") {
        let n = be32(&head, 8).unwrap_or(0).min(MAX_FACES) as usize;
        read_at(12, n * 4).map(|b| b.as_chunks::<4>().0.iter().map(|c| u32::from_be_bytes(*c)).collect()).unwrap_or_default()
    } else {
        vec![0]
    };
    starts
        .into_iter()
        .zip(0..)
        .filter_map(|(start, index)| {
            let dir = read_at(start.into(), 12)?;
            let tables = u16::from_be_bytes(dir.get(4..6)?.try_into().ok()?) as usize;
            let records = read_at(u64::from(start) + 12, tables * 16)?;
            // Faces drawn from other tables (Apple's `hvgl`, bitmaps) would draw nothing.
            if !records.as_chunks::<16>().0.iter().any(|r| OUTLINE_TABLES.iter().any(|t| r.starts_with(*t))) {
                return None;
            }
            // Table `tag`, unless missing or over `max` bytes.
            let mut table = |tag: &[u8; 4], max: u32| -> Option<([u8; 4], Vec<u8>)> {
                let rec = records.as_chunks::<16>().0.iter().find(|r| r.starts_with(tag))?;
                let (offset, len) = (be32(rec, 8)?, be32(rec, 12)?);
                if len > max {
                    return None;
                }
                Some((*tag, read_at(offset.into(), len as usize)?))
            };
            // A font holding just these tables, to read them as the font itself would.
            let mut found = vec![table(b"name", MAX_NAME_TABLE)?];
            found.extend(table(b"OS/2", MAX_SMALL_TABLE));
            found.extend(table(b"meta", MAX_SMALL_TABLE));
            let font = crate::group::sfnt(found.clone())?;
            let f = skrifa::FontRef::new(&font).ok()?;
            let (family, style) = face_names(&f)?;
            let (group, native) = match crate::group::declared_group(&f, &family) {
                Some(g) => (g, crate::group::classify(&f, &family).1),
                None => {
                    found.extend(table(b"cmap", MAX_CMAP));
                    let font = crate::group::sfnt(found)?;
                    crate::group::classify(&skrifa::FontRef::new(&font).ok()?, &family)
                }
            };
            Some(ScannedFace {
                index,
                wws: wws_names(&f, &family, &style),
                group,
                native,
                aliases: aliases(&f, &[StringId::TYPOGRAPHIC_FAMILY_NAME, StringId::FAMILY_NAME]),
            })
        })
        .collect()
}

/// The platform's font folders (the system's and the user's), which [`FontDb::global`]'s catalog
/// scans. Empty on wasm.
pub fn system_font_dirs() -> Vec<std::path::PathBuf> {
    #[cfg(not(target_arch = "wasm32"))]
    return craft_font_catalog::font_dirs(&craft_font_catalog::FontEnv::from_process());
    #[cfg(target_arch = "wasm32")]
    Vec::new()
}

impl FontDb {
    /// Whether the installed fonts are listed: a lookup by family name won't wait for the catalog's
    /// scan (always true on wasm, which has none). See [`FontDb::scan_in_background`].
    pub fn system_fonts_cataloged(&self) -> bool {
        #[cfg(target_arch = "wasm32")]
        return true;
        #[cfg(not(target_arch = "wasm32"))]
        self.listing.read().unwrap_or_else(PoisonError::into_inner).is_some()
    }

    /// List the installed fonts on a background thread, so the first lookup by family name
    /// (opening a file, the font menus) doesn't wait for the catalog's scan. A no-op once they are
    /// listed, and on wasm.
    pub fn scan_in_background(&'static self) {
        #[cfg(not(target_arch = "wasm32"))]
        if !self.system_fonts_cataloged() {
            // A failed spawn leaves the scan to the first lookup that needs it.
            let _ = std::thread::Builder::new().name("font-scan".into()).spawn(move || self.read_catalog());
        }
    }

    /// The app's window was activated: look for fonts installed or removed since (the launcher's
    /// Fonts tab, the system's font manager) on a worker thread ([`craft_font_catalog::app::on_activated`]).
    /// When it finds some, the database lists them there too, then calls the waker
    /// ([`FontDb::set_change_waker`]) for the app to take them ([`FontDb::refresh`]). Nothing on
    /// wasm.
    pub fn window_activated() {
        #[cfg(not(target_arch = "wasm32"))]
        let _ = craft_font_catalog::app::on_activated();
    }

    /// Call `wake` on the worker thread once a rescan has changed the installed fonts and they are
    /// listed ([`FontDb::change_listed`]), so an idle window wakes to take them. The first waker
    /// set stays.
    pub fn set_change_waker(wake: impl Fn() + Send + Sync + 'static) {
        let _ = WAKER.set(Box::new(wake));
    }

    /// A rescan on a worker thread changed the installed fonts: list them there.
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn changed_in_background(&self) {
        self.list_ahead();
        if let Some(wake) = WAKER.get() {
            wake();
        }
    }

    /// Whether the fonts changed and the next listing is made ([`FontDb::window_activated`]):
    /// [`FontDb::refresh`] then takes it without reading font files on the caller's thread.
    pub fn change_listed(&self) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        return self.pending.lock().unwrap_or_else(PoisonError::into_inner).as_ref().is_some_and(|(g, _)| *g == self.catalog.generation());
        #[cfg(target_arch = "wasm32")]
        false
    }

    /// Whether the catalog changed since the listing lookups see was made: [`FontDb::refresh`]
    /// takes the change. False before the first listing.
    pub fn fonts_changed(&self) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        return self.listing.read().unwrap_or_else(PoisonError::into_inner).as_ref().is_some_and(|(g, _)| *g != self.catalog.generation());
        #[cfg(target_arch = "wasm32")]
        false
    }

    /// Take the fonts installed, changed or removed since the last listing: lookups see the
    /// catalog as it is now, and the faces made from font files changed or removed since are let
    /// go (see [`ReloadedFonts`]), so what was composed with them must be composed again. Faces
    /// are never freed (see [`FaceRef`]): each reload of a changed file keeps the old copy.
    /// Bundled, added and document fonts are left alone. Nothing to do on wasm.
    pub fn refresh(&self) -> ReloadedFonts {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let (cataloged, since) = {
                let _made = self.listing_made.lock().unwrap_or_else(PoisonError::into_inner);
                let now = self.catalog.generation();
                let ahead = self.pending.lock().unwrap_or_else(PoisonError::into_inner).take().filter(|(g, _)| *g == now);
                let next = ahead.unwrap_or_else(|| self.make_listing());
                let n = next.1.len();
                let was = self.listing.write().unwrap_or_else(PoisonError::into_inner).replace(next);
                (n, was.map(|(g, _)| g))
            };
            let changes = since.map(|g| self.catalog.changes_since(g)).unwrap_or_default();
            let mut changed = Vec::new();
            let mut families: Vec<String> = changes.removed.into_iter().chain(changes.changed).collect();
            let mut dropped = std::collections::HashSet::new();
            for (key, faces) in self.installed.refresh(&self.catalog) {
                if let FaceKey::File(path, _) = key {
                    changed.push(path);
                }
                for f in faces.iter() {
                    dropped.insert(f.id);
                    families.push(f.family.clone());
                }
            }
            changed.sort();
            changed.dedup();
            families.sort_by_key(|f| f.to_lowercase());
            families.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
            if !dropped.is_empty() {
                self.faces.write().unwrap_or_else(PoisonError::into_inner).retain(|f| !dropped.contains(&f.id));
                self.instances.write().unwrap_or_else(PoisonError::into_inner).retain(|(base, _), f| {
                    let keep = !dropped.contains(base);
                    if !keep {
                        dropped.insert(f.id);
                    }
                    keep
                });
                self.outlines.lock().unwrap_or_else(PoisonError::into_inner).retain(|(face, _), _| !dropped.contains(face));
                log::debug!("let go of {} faces of {} changed or removed font files", dropped.len(), changed.len());
            }
            // Retry characters and CJK chain families that the fonts installed now can cover. Not
            // while making the listing: a chain lookup holds `sys` while it waits for one.
            let mut sys = self.sys.lock().unwrap_or_else(PoisonError::into_inner);
            sys.generation = sys.generation.wrapping_add(1);
            sys.misses.clear();
            sys.chain_tried.clear();
            ReloadedFonts { cataloged, changed, families, full: changes.full }
        }
        #[cfg(target_arch = "wasm32")]
        ReloadedFonts::default()
    }

    /// Rescan the catalog (fonts installed or removed since; only new and changed files are read)
    /// and take what it finds ([`FontDb::refresh`]). Font data is loaded when a family is first
    /// resolved. Returns the number of installed faces listed.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load_system_fonts(&self) -> usize {
        self.reload_fonts().cataloged
    }

    /// Fonts installed, changed or removed since they were read (#328): rescan the catalog fully,
    /// then take what it finds ([`FontDb::refresh`]). Nothing to do on wasm.
    pub fn reload_fonts(&self) -> ReloadedFonts {
        #[cfg(not(target_arch = "wasm32"))]
        if self.app_catalog {
            craft_font_catalog::app::rescan_now();
        } else {
            self.catalog.rescan();
        }
        self.refresh()
    }

    /// The first listing: made here, or by the thread already making it.
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn first_listing(&self) -> Arc<[CatalogEntry]> {
        let _made = self.listing_made.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some((_, entries)) = &*self.listing.read().unwrap_or_else(PoisonError::into_inner) {
            return Arc::clone(entries);
        }
        let listing = self.make_listing();
        let entries = Arc::clone(&listing.1);
        *self.listing.write().unwrap_or_else(PoisonError::into_inner) = Some(listing);
        entries
    }

    /// Make the listing of the catalog as it is now ahead of [`FontDb::refresh`], unless it is
    /// made already.
    #[cfg(not(target_arch = "wasm32"))]
    fn list_ahead(&self) {
        let _made = self.listing_made.lock().unwrap_or_else(PoisonError::into_inner);
        let now = self.catalog.generation();
        let listed = self.listing.read().unwrap_or_else(PoisonError::into_inner).as_ref().map(|(g, _)| *g);
        let mut pending = self.pending.lock().unwrap_or_else(PoisonError::into_inner);
        if listed.is_some() && listed != Some(now) && pending.as_ref().is_none_or(|(g, _)| *g != now) {
            *pending = Some(self.make_listing());
        }
    }

    /// The catalog's faces as they are now, with what the catalog doesn't give read from the font
    /// files new or changed since the last listing. Bundled faces are left out: they are loaded.
    #[cfg(not(target_arch = "wasm32"))]
    fn make_listing(&self) -> Listing {
        if !self.catalog.is_ready() {
            // The first scan, so that the generation read next is the one the faces have.
            let _ = self.catalog.faces();
        }
        // Read before the faces: a change while they are listed shows as a newer generation.
        let generation = self.catalog.generation();
        let faces = self.catalog.faces();
        let mut scanned = self.scanned.lock().unwrap_or_else(PoisonError::into_inner);
        let mut kept: HashMap<std::path::PathBuf, (FileStamp, Arc<[ScannedFace]>)> = HashMap::new();
        let mut entries = Vec::new();
        for info in faces.iter().filter(|f| f.source != Source::Bundled) {
            // A face added as bytes has no file to read more of.
            let own = match info.path.as_deref() {
                Some(path) => {
                    if !kept.contains_key(path) {
                        let Some(stamp) = file_stamp(path) else { continue };
                        let read = match scanned.remove(path) {
                            Some((was, read)) if was == stamp => read,
                            _ => file_face_names(path).into(),
                        };
                        kept.insert(path.to_path_buf(), (stamp, read));
                    }
                    kept.get(path).and_then(|(_, read)| read.iter().find(|f| f.index == info.index).cloned())
                }
                None => None,
            };
            entries.push(CatalogEntry {
                family: info.family.clone(),
                style: info.style.clone(),
                // The WWS names read are the default instance's.
                wws: own.as_ref().filter(|_| info.variations.is_empty()).and_then(|f| f.wws.clone()),
                group: own.as_ref().map_or(FontGroup::Western, |f| f.group),
                native: own.as_ref().and_then(|f| f.native.clone()),
                aliases: own.map_or_else(|| catalog_aliases(info), |f| f.aliases),
                postscript: info.postscript_name.clone(),
                info: info.clone(),
            });
        }
        *scanned = kept;
        log::debug!("listed {} installed font faces", entries.len());
        (generation, entries.into())
    }

    /// Load the installed `family`'s faces. Returns whether any face was added.
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn load_cataloged(&self, family: &str) -> bool {
        let mut infos: Vec<FaceInfo> = self.read_catalog().iter().filter(|c| in_catalog_family(c, family)).map(|c| c.info.clone()).collect();
        infos.sort_by(|a, b| (&a.path, a.index).cmp(&(&b.path, b.index)));
        infos.dedup_by(|a, b| (&a.path, a.index) == (&b.path, b.index));
        // The whole family arrives at once (see `insert_faces`).
        let faces: Vec<Arc<FontFace>> = infos.iter().filter_map(|info| self.installed_faces(info)).flat_map(|f| f.to_vec()).collect();
        self.insert_faces(faces) > 0
    }

    /// The faces of the installed face `info` (one per named instance of a variable font), made
    /// once per font file and face index.
    #[cfg(not(target_arch = "wasm32"))]
    fn installed_faces(&self, info: &FaceInfo) -> Option<Arc<Vec<Arc<FontFace>>>> {
        let source = installed_source(info);
        let made = self.installed.get_or_make(&self.catalog, info, |_, data| {
            let faces: Vec<Arc<FontFace>> = faces_at(&data.bytes, data.index)
                .into_iter()
                .filter_map(|(i, family, style, coords)| {
                    make_face(FontBytes::Shared(Arc::clone(&data.bytes)), source.clone(), i, family, style, coords)
                })
                .map(Arc::new)
                .collect();
            (!faces.is_empty()).then_some(faces)
        });
        match made {
            Ok(faces) => Some(faces),
            Err(e) => {
                log::debug!("{} {}: {e}", info.family, info.style);
                None
            }
        }
    }

    /// Load the font files (.ttf, .otf, .ttc, .otc; not in subfolders) of a document's `Document
    /// Fonts` folder into a new scope of the catalog, for that document alone: looked up through
    /// [`FontDb::scoped`], they come before the shared faces of the same family and style, until
    /// [`FontDb::close_scope`]. A file loaded before and unchanged since (same length and
    /// modification time) isn't loaded again: its faces are shared, and the catalog reads only its
    /// tables. The files are untrusted: past [`MAX_DOCUMENT_FONT_FILES`] (in name order), files over [`MAX_DOCUMENT_FONT_BYTES`] or past
    /// [`MAX_DOCUMENT_FONTS_TOTAL`] in all, and files that aren't fonts are skipped with a note. A
    /// missing folder loads nothing (scope 0).
    ///
    /// The folder is named by a document, so this crate does not trust it: `allow` (`Ok`, or why a
    /// path may not be read) is asked about
    /// the folder and about every font file in it before either is opened, and a path it refuses
    /// is skipped with its message (the engine passes its automation-root check; a host with no
    /// roots passes `&|_| Ok(())`).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load_document_fonts(&self, folder: &std::path::Path, allow: &dyn Fn(&std::path::Path) -> Result<(), String>) -> DocumentFonts {
        let mut out = DocumentFonts::default();
        if let Err(why) = allow(folder) {
            out.skipped.push(why);
            return out;
        }
        let Ok(entries) = std::fs::read_dir(folder) else { return out };
        let mut files: Vec<std::path::PathBuf> = Vec::new();
        for path in entries.flatten().map(|e| e.path()) {
            let ext = path.extension().and_then(std::ffi::OsStr::to_str).map(str::to_ascii_lowercase);
            if !matches!(ext.as_deref(), Some("ttf" | "otf" | "ttc" | "otc")) {
                continue;
            }
            // Each file is vetted before it is touched at all, a link out of the folder included.
            match allow(&path) {
                Ok(()) if path.is_file() => files.push(path),
                Ok(()) => {}
                Err(why) => out.skipped.push(why),
            }
        }
        files.sort();
        let scope = self.catalog.new_scope();
        let mut total = 0u64;
        let mut own: Vec<Arc<FontFace>> = Vec::new();
        for (n, path) in files.iter().enumerate() {
            if n == MAX_DOCUMENT_FONT_FILES {
                out.skipped.push(format!("{} more font files not loaded (at most {MAX_DOCUMENT_FONT_FILES} are)", files.len() - n));
                break;
            }
            let name = path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
            // The listed length skips oversized files unread.
            let listed = std::fs::metadata(path).map_or(0, |m| m.len());
            let faces = if listed > MAX_DOCUMENT_FONT_BYTES {
                Err(too_large())
            } else if total.saturating_add(listed) > MAX_DOCUMENT_FONTS_TOTAL {
                Err(over_total())
            } else {
                self.document_file(scope, path)
            };
            match faces {
                Err(r) => out.skipped.push(format!("{name}: {r}")),
                Ok(faces) => {
                    total = total.saturating_add(listed);
                    // A family and style the folder already gave stays with its first file.
                    for f in faces {
                        if !own.iter().any(|o| o.family.eq_ignore_ascii_case(&f.family) && o.style.eq_ignore_ascii_case(&f.style)) {
                            own.push(f);
                        }
                    }
                }
            }
        }
        for s in &out.skipped {
            log::warn!("{}: {s}", folder.display());
        }
        if own.is_empty() {
            self.catalog.remove_scope(scope);
        } else {
            out.scope = NEXT_SCOPE.fetch_add(1, Ordering::Relaxed);
            out.faces = own.len();
            self.scopes.write().unwrap_or_else(PoisonError::into_inner).insert(out.scope, DocScope { catalog: scope, faces: own.into() });
        }
        out
    }

    /// Add the document font file at `path` to the catalog's `scope`, and make its faces: those
    /// made before if the file hasn't changed since, else read now (documents open before keep
    /// the faces they opened with) → the faces, or why the file is skipped.
    #[cfg(not(target_arch = "wasm32"))]
    fn document_file(&self, scope: craft_font_catalog::Scope, path: &std::path::Path) -> Result<Vec<Arc<FontFace>>, String> {
        let limits = craft_font_catalog::Limits {
            max_file_bytes: MAX_DOCUMENT_FONT_BYTES,
            max_added_faces: MAX_ADDED_FACES,
            ..craft_font_catalog::Limits::default()
        };
        self.documents.forget_changed(path);
        let mut infos = self.catalog.add_file_in(scope, path, Source::Document, &limits).map_err(|e| document_font_error(&e))?;
        // A variable font's named instances share their face's number: its faces are made at once.
        infos.sort_by_key(|i| i.index);
        infos.dedup_by_key(|i| i.index);
        let source = FontSource::Document(path.to_path_buf());
        let mut faces = Vec::new();
        for info in &infos {
            let made = self.documents.get_or_make(&self.catalog, info, |_, data| {
                // The file may have grown since it was listed.
                if data.bytes.len() as u64 > MAX_DOCUMENT_FONT_BYTES {
                    return None;
                }
                let faces: Vec<Arc<FontFace>> = faces_at(&data.bytes, data.index)
                    .into_iter()
                    .filter_map(|(i, family, style, coords)| {
                        make_face(FontBytes::Shared(Arc::clone(&data.bytes)), source.clone(), i, family, style, coords)
                    })
                    .map(Arc::new)
                    .collect();
                (!faces.is_empty()).then_some(faces)
            });
            match made {
                Ok(made) => faces.extend(made.iter().cloned()),
                Err(e) => log::debug!("{} {}: {e}", info.family, info.style),
            }
        }
        if faces.is_empty() {
            return Err(document_font_error(&craft_font_catalog::FontFileError::NotAFont));
        }
        Ok(faces)
    }

    /// Load a system font covering `c` (preferred fallback families first, then any installed
    /// file under 40 MB). Returns true if one was loaded. Misses are remembered.
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn system_fallback(&self, c: char) -> bool {
        if c.is_control() || c.is_whitespace() {
            return false;
        }
        let generation = {
            let sys = self.sys.lock().unwrap_or_else(PoisonError::into_inner);
            if !sys.enabled || sys.misses.contains(&c) {
                return false;
            }
            sys.generation
        };
        let covered = |db: &FontDb| db.read_faces().iter().any(|f| f.covers(c));
        let listing = self.read_catalog();
        for fam in SYSTEM_FALLBACKS {
            if !self.is_loaded(fam) && listing.iter().any(|e| e.family.eq_ignore_ascii_case(fam)) {
                self.load_cataloged(fam);
                if covered(self) {
                    return true;
                }
            }
        }
        let mut infos: Vec<&FaceInfo> = listing.iter().map(|e| &e.info).collect();
        infos.sort_by(|a, b| (&a.path, a.index).cmp(&(&b.path, b.index)));
        infos.dedup_by(|a, b| (&a.path, a.index) == (&b.path, b.index));
        #[cfg(all(test, target_os = "linux"))]
        {
            let hook = self.fallback_snapshot_hook.lock().unwrap_or_else(PoisonError::into_inner).take();
            if let Some(hook) = hook {
                let _ = hook.reached.send(());
                hook.resume.wait();
            }
        }
        for info in infos {
            if info.path.as_deref().is_some_and(|path| std::fs::metadata(path).map(|m| m.len() > 40 << 20).unwrap_or(true)) {
                continue;
            }
            let Ok(data) = self.catalog.load(info) else { continue };
            if data.bytes.len() > 40 << 20 {
                continue;
            }
            if !skrifa::FontRef::from_index(&data.bytes, data.index).is_ok_and(|f| f.charmap().map(c).is_some()) {
                continue;
            }
            if let Some(faces) = self.installed_faces(info)
                && self.insert_faces(faces.to_vec()) > 0
                && covered(self)
            {
                return true;
            }
        }
        let mut sys = self.sys.lock().unwrap_or_else(PoisonError::into_inner);
        if sys.generation == generation {
            sys.misses.insert(c);
        }
        false
    }
}

/// Where an installed face's font comes from: its file, else bytes added to the catalog.
#[cfg(not(target_arch = "wasm32"))]
fn installed_source(info: &FaceInfo) -> FontSource {
    info.path.clone().map_or(FontSource::Memory, FontSource::Installed)
}

/// The names `info`'s family is known by, as the catalog read them.
#[cfg(not(target_arch = "wasm32"))]
fn catalog_aliases(info: &FaceInfo) -> Vec<String> {
    let mut names: Vec<String> = std::iter::once(&info.family)
        .chain(&info.legacy_family)
        .chain(info.localized_families.iter().map(|l| &l.name))
        .filter(|n| !n.trim().is_empty())
        .cloned()
        .collect();
    names.sort();
    names.dedup();
    names
}

impl ScopedFonts<'_> {
    /// `family`'s kind (format, class, PANOSE) and when its file was added, read from its Regular
    /// face (else its first), for the font menus' filters, which look at every family. A face
    /// already loaded is asked; an installed family that isn't loaded is read from its file, which
    /// is then let go rather than kept loaded. None for an unknown family or an unreadable file.
    pub fn family_kind(&self, family: &str) -> Option<(crate::kind::FaceKind, Option<std::time::SystemTime>)> {
        let styles = self.styles(family);
        let style = styles.iter().find(|s| *s == "Regular").or(styles.first())?.clone();
        if self.own().iter().chain(self.db.read_faces().iter()).any(|f| f.family.eq_ignore_ascii_case(family)) {
            let f = self.find(family, &style)?;
            return Some((f.kind(), f.added()));
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let info = {
                let listing = self.db.read_catalog();
                let of_family = |c: &&CatalogEntry| c.family.eq_ignore_ascii_case(family);
                let entry = listing.iter().filter(of_family).find(|c| norm(&c.style) == norm(&style)).or_else(|| listing.iter().find(of_family));
                entry.map(|c| c.info.clone())?
            };
            if let Some(path) = &info.path
                && std::fs::metadata(path).ok()?.len() > MAX_DOCUMENT_FONT_BYTES
            {
                return None;
            }
            let data = self.db.catalog.load(&info).ok()?;
            let found = faces_at(&data.bytes, data.index);
            let pick = found
                .iter()
                .find(|(_, f, st, _)| f.eq_ignore_ascii_case(family) && norm(st) == norm(&style))
                .or_else(|| found.iter().find(|(_, f, _, _)| f.eq_ignore_ascii_case(family)))?
                .clone();
            let (index, fam, st, coords) = pick;
            let face = make_face(FontBytes::Shared(data.bytes), installed_source(&info), index, fam, st, coords)?;
            Some((face.kind(), face.added()))
        }
        #[cfg(target_arch = "wasm32")]
        None
    }
}

/// One open document's fonts: the catalog scope its `Document Fonts` folder was added to, and the
/// faces made from it, in the folder's file order.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone)]
pub(crate) struct DocScope {
    pub(super) catalog: craft_font_catalog::Scope,
    pub(super) faces: Arc<[Arc<FontFace>]>,
}

/// The most faces added to the catalog in all, every open document's fonts and the bundled ones
/// included: [`MAX_DOCUMENT_FONT_FILES`] collections of 256 faces each, for a few documents.
#[cfg(not(target_arch = "wasm32"))]
const MAX_ADDED_FACES: usize = 1 << 18;

/// Why a document font file is skipped, as the note says it.
#[cfg(not(target_arch = "wasm32"))]
fn document_font_error(e: &craft_font_catalog::FontFileError) -> String {
    use craft_font_catalog::FontFileError;
    match e {
        FontFileError::Io(e) => format!("can't be read ({e})"),
        FontFileError::TooLarge { .. } => too_large(),
        FontFileError::NotAFont | FontFileError::Gone => "not a font DesignCraft can read".into(),
        FontFileError::TooManyFaces { max } => format!("more than {max} font faces are open"),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn too_large() -> String {
    format!("larger than {} MB", MAX_DOCUMENT_FONT_BYTES >> 20)
}

#[cfg(not(target_arch = "wasm32"))]
fn over_total() -> String {
    format!("over the {} MB the folder's fonts may take in all", MAX_DOCUMENT_FONTS_TOTAL >> 20)
}
