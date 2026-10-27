use super::face::{BUNDLED, FontBytes, FontFace, FontSource, enumerate_faces, make_face};
#[cfg(not(target_arch = "wasm32"))]
use super::fallback::SysFallback;
use super::names::has_face;
#[cfg(not(target_arch = "wasm32"))]
use super::names::has_wws_family;
use super::outline::font_outline;
use super::scoped::ScopedFonts;
#[cfg(not(target_arch = "wasm32"))]
use super::system::CatalogEntry;
use super::*;

pub(super) static NEXT_ID: AtomicU32 = AtomicU32::new(1);

/// Scopes are never reused: a closed document's scope finds no fonts ever after.
#[cfg(not(target_arch = "wasm32"))]
pub(super) static NEXT_SCOPE: AtomicU32 = AtomicU32::new(1);

const OUTLINE_CACHE_MAX: usize = 50_000;

impl FontDb {
    /// A database holding the bundled fonts, then the craft-fonts Japanese faces (Mincho first),
    /// followed by Noto Sans Hebrew Regular and Noto Sans Devanagari Regular for document text.
    /// The optional craft-fonts faces are absent without `CRAFT_FONTS_DIR`. The installed fonts
    /// are those in `font_dirs` alone (no platform folders, font service or font store; nothing on
    /// wasm). Japanese fallback and UI ordering remain independent of these additions.
    pub fn with_font_dirs(font_dirs: Vec<std::path::PathBuf>) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let catalog = craft_font_catalog::Catalog::without_installed_fonts();
            catalog.set_app_dirs(font_dirs);
            Self::with_catalog(Arc::new(catalog))
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = font_dirs;
            Self::bundled_only()
        }
    }

    /// A database holding the bundled fonts (see [`FontDb::with_font_dirs`]), with `catalog`'s
    /// fonts installed.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn with_catalog(catalog: Arc<craft_font_catalog::Catalog>) -> Self {
        Self {
            faces: RwLock::new(Self::bundled_faces()),
            scopes: RwLock::new(HashMap::new()),
            closed_scopes: Mutex::new(HashMap::new()),
            documents: super::catalog_faces::FaceCache::new(),
            instances: RwLock::new(HashMap::new()),
            outlines: Mutex::new(HashMap::new()),
            catalog,
            listing: RwLock::new(None),
            pending: Mutex::new(None),
            listing_made: Mutex::new(()),
            scanned: Mutex::new(HashMap::new()),
            installed: super::catalog_faces::FaceCache::new(),
            app_catalog: false,
            sys: Mutex::new(SysFallback { enabled: true, ..Default::default() }),
            #[cfg(all(test, target_os = "linux"))]
            fallback_snapshot_hook: Mutex::new(None),
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn bundled_only() -> Self {
        Self { faces: RwLock::new(Self::bundled_faces()), instances: RwLock::new(HashMap::new()), outlines: Mutex::new(HashMap::new()) }
    }

    /// The font files of the faces every database starts with, in order.
    pub(super) fn document_font_files() -> impl Iterator<Item = &'static [u8]> {
        let craft = crate::japanese_document_fonts()
            .into_iter()
            .chain(
                ["Noto Sans Hebrew", "Noto Sans Devanagari"]
                    .into_iter()
                    .filter_map(|family| crate::CRAFT_FONTS.iter().find(|f| f.family == family && f.style == "Regular")),
            )
            .map(|f| f.bytes);
        BUNDLED.iter().copied().chain(craft)
    }

    fn bundled_faces() -> Vec<Arc<FontFace>> {
        let mut faces = Vec::new();
        for data in Self::document_font_files() {
            for (i, family, style, coords) in enumerate_faces(data) {
                if let Some(f) = make_face(FontBytes::Static(data), FontSource::Bundled, i, family, style, coords) {
                    faces.push(Arc::new(f));
                }
            }
        }
        faces
    }

    /// Enable or disable the lazy system-font fallback for characters the loaded fonts lack
    /// (native only; on by default).
    pub fn set_system_fallback(&self, on: bool) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.sys.lock().unwrap_or_else(PoisonError::into_inner).enabled = on;
        }
        #[cfg(target_arch = "wasm32")]
        let _ = on;
    }

    /// Process-wide database: the bundled fonts, then the installed fonts of the process's font
    /// catalog ([`craft_font_catalog::Catalog::global`], configured by
    /// [`craft_font_catalog::app::configure`]: the platform's font folders and font service, the
    /// suite's font store, and Office's own fonts), listed the first time a lookup by family name
    /// needs them (or ahead of time by [`FontDb::scan_in_background`]).
    pub fn global() -> &'static FontDb {
        static DB: std::sync::OnceLock<FontDb> = std::sync::OnceLock::new();
        DB.get_or_init(|| {
            #[cfg(not(target_arch = "wasm32"))]
            {
                use craft_font_catalog::Catalog;
                use craft_font_catalog::app::{self, AppFonts, Office};
                // Office's fonts (Calibri, Cambria, …) are read in place from the Office apps. Not
                // its cloud fonts, whose folder is in Office's group container, which macOS asks
                // the user before another app reads.
                let started = app::configure(AppFonts {
                    office: Office::Bundled,
                    bundled: Self::document_font_files().map(Arc::from).collect(),
                    on_change: Some(Arc::new(|_: &Catalog| FontDb::global().changed_in_background())),
                    ..AppFonts::default()
                });
                for (_, e) in &started.bundled_errors {
                    log::warn!("a bundled font isn't in the font catalog: {e}");
                }
                FontDb { app_catalog: true, ..FontDb::with_catalog(Arc::clone(started.catalog)) }
            }
            #[cfg(target_arch = "wasm32")]
            FontDb::bundled_only()
        })
    }

    pub(super) fn read_faces(&self) -> std::sync::RwLockReadGuard<'_, Vec<Arc<FontFace>>> {
        self.faces.read().unwrap_or_else(PoisonError::into_inner)
    }

    /// The installed faces as lookups see them, listed first if they haven't been yet.
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn read_catalog(&self) -> Arc<[CatalogEntry]> {
        if let Some((_, entries)) = &*self.listing.read().unwrap_or_else(PoisonError::into_inner) {
            return Arc::clone(entries);
        }
        self.first_listing()
    }

    /// What the document with font scope `scope` sees: its own fonts ahead of the shared ones.
    /// Scope 0 (a document without fonts of its own) or a closed scope sees the shared fonts.
    pub fn scoped(&self, scope: u32) -> ScopedFonts<'_> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let own = if scope == 0 { None } else { self.scopes.read().unwrap_or_else(PoisonError::into_inner).get(&scope).cloned() };
            ScopedFonts { db: self, own: own.as_ref().map(|s| Arc::clone(&s.faces)), scope: own.map(|s| s.catalog) }
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = scope;
            ScopedFonts { db: self, own: None }
        }
    }

    /// How often the fonts of document font scope `scope` changed: the scope's part of the
    /// catalog's [`craft_font_catalog::Catalog::generation_in`], without the shared generation
    /// ([`FontDb::refresh`] takes shared changes, and the engine composes again only the documents
    /// they affect). Layout caches key on it, so one document's fonts loading or closing doesn't
    /// invalidate another's. A closed scope keeps the value it had when it closed, so the layouts
    /// composed in it stay cached for the jobs that still hold them. 0 for scope 0 and on wasm.
    pub fn scope_generation(&self, scope: u32) -> u64 {
        #[cfg(not(target_arch = "wasm32"))]
        {
            if scope == 0 {
                return 0;
            }
            let open = self.scopes.read().unwrap_or_else(PoisonError::into_inner).get(&scope).map(|s| s.catalog);
            match open {
                Some(own) => self.own_generation(own),
                None => self.closed_scopes.lock().unwrap_or_else(PoisonError::into_inner).get(&scope).copied().unwrap_or(0),
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = scope;
            0
        }
    }

    /// The catalog scope `own`'s part of its generation.
    #[cfg(not(target_arch = "wasm32"))]
    fn own_generation(&self, own: craft_font_catalog::Scope) -> u64 {
        // `generation_in` adds the shared generation, which a rescan may move on meanwhile.
        for _ in 0..8 {
            let shared = self.catalog.generation();
            let all = self.catalog.generation_in(own);
            if self.catalog.generation() == shared {
                return all.saturating_sub(shared);
            }
        }
        // Rescans kept moving it on: a key that also changes with the shared generation is still
        // a key, it only lays out again more often.
        self.catalog.generation_in(own)
    }

    /// The shared fonts' family names (see [`ScopedFonts::families`]).
    pub fn families(&self) -> Vec<String> {
        self.scoped(0).families()
    }

    /// The shared fonts' styles of `family` (see [`ScopedFonts::styles`]).
    pub fn styles(&self, family: &str) -> Vec<String> {
        self.scoped(0).styles(family)
    }

    /// The shared fonts' weight and italic of a style of `family` (see [`ScopedFonts::traits_of`]).
    pub fn traits_of(&self, family: &str, style: &str) -> (f32, bool) {
        self.scoped(0).traits_of(family, style)
    }

    /// Style linking among the shared fonts (see [`ScopedFonts::linked_style`]).
    pub fn linked_style(&self, family: &str, current: &str, bold: Option<bool>, italic: Option<bool>) -> Option<String> {
        self.scoped(0).linked_style(family, current, bold, italic)
    }

    /// Add a user font (TTF/OTF/TTC bytes). Returns the number of faces added (0 if unparseable or
    /// every face was already present).
    pub fn add_font(&self, bytes: Vec<u8>) -> usize {
        self.add_faces(bytes, FontSource::Memory)
    }

    /// Add the faces of a font file from `source` to the shared fonts, skipping the family and
    /// style already loaded.
    pub(super) fn add_faces(&self, bytes: Vec<u8>, source: FontSource) -> usize {
        let faces = self.new_faces(bytes, source);
        self.insert_faces(faces.into_iter().map(Arc::new).collect())
    }

    /// The faces of a font file from `source` whose family and style aren't loaded yet.
    pub(crate) fn new_faces(&self, bytes: Vec<u8>, source: FontSource) -> Vec<FontFace> {
        let data = Arc::new(bytes);
        enumerate_faces(&data)
            .into_iter()
            .filter(|(_, family, style, _)| !has_face(&self.read_faces(), family, style))
            .filter_map(|(i, family, style, coords)| make_face(FontBytes::Owned(data.clone()), source.clone(), i, family, style, coords))
            .collect()
    }

    /// Add `faces` to the shared fonts all at once, so a lookup sees all of them or none (not a
    /// family's bold without its regular), skipping a family and style another thread has added
    /// meanwhile. Returns the number added.
    pub(super) fn insert_faces(&self, new: Vec<Arc<FontFace>>) -> usize {
        let mut faces = self.faces.write().unwrap_or_else(PoisonError::into_inner);
        let mut added = 0;
        for f in new {
            if !has_face(&faces, &f.family, &f.style) {
                faces.push(f);
                added += 1;
            }
        }
        added
    }

    /// Close a document's font scope: its fonts are no longer found (in it or anywhere), and the
    /// catalog's scope is removed. The faces made from its files stay, for the document opening
    /// again. Nothing to do on wasm, which has no document fonts.
    pub fn close_scope(&self, scope: u32) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let closed = self.scopes.write().unwrap_or_else(PoisonError::into_inner).remove(&scope);
            if let Some(own) = closed {
                // What the document composed with stays cached for the jobs that still hold it.
                let generation = self.own_generation(own.catalog);
                self.closed_scopes.lock().unwrap_or_else(PoisonError::into_inner).insert(scope, generation);
                self.catalog.remove_scope(own.catalog);
            }
        }
        #[cfg(target_arch = "wasm32")]
        let _ = scope;
    }

    /// The shared fonts' face for `family` + `style` (see [`ScopedFonts::face`]).
    pub fn face(&self, family: &str, style: &str) -> Arc<FontFace> {
        self.scoped(0).face(family, style)
    }

    /// The shared face with PostScript name `name` (see [`ScopedFonts::postscript_face`]).
    pub fn postscript_face(&self, name: &str) -> Option<(String, String)> {
        self.scoped(0).postscript_face(name)
    }

    /// Is `family` among the shared fonts (loaded, or installed on the system)?
    pub fn has_family(&self, family: &str) -> bool {
        self.scoped(0).has_family(family)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn is_loaded(&self, family: &str) -> bool {
        self.read_faces().iter().any(|f| f.family.eq_ignore_ascii_case(family) || has_wws_family(f, family))
    }

    /// A shared variable font's axes (see [`ScopedFonts::axes`]).
    pub fn axes(&self, family: &str, style: &str) -> Vec<(String, String, f32, f32, f32)> {
        self.scoped(0).axes(family, style)
    }

    /// A shared face other than `exclude` that covers `c` (see [`ScopedFonts::fallback_for`]).
    pub fn fallback_for(&self, c: char, exclude: u32, language: Option<&str>) -> Option<Arc<FontFace>> {
        self.scoped(0).fallback_for(c, exclude, language)
    }

    /// The box drawn for `face`'s missing glyph (.notdef) when the font's own has no outline, in
    /// font units, y-down: a hollow rectangle across the glyph's advance, cap height tall. `None`
    /// when the font draws its own.
    pub fn missing_box(&self, face: &FontFace) -> Option<BezPath> {
        if !font_outline(face, 0).elements().is_empty() {
            return None;
        }
        let adv = Some(face.advance(0)).filter(|a| a.is_finite() && *a > 0.0).unwrap_or(face.upem * 0.5);
        let top = Some(face.cap_height).filter(|h| h.is_finite() && *h > 0.0).unwrap_or(face.upem * 0.7);
        let outer = kurbo::Rect::new(adv * 0.1, -top, adv * 0.9, 0.0);
        let t = (face.upem * 0.05).min(outer.width() / 4.0).min(top / 4.0);
        let inner = outer.inset(-t);
        let mut p = BezPath::new();
        // The inner rectangle runs the other way round, so filling leaves it empty.
        p.move_to((outer.x0, outer.y0));
        p.line_to((outer.x1, outer.y0));
        p.line_to((outer.x1, outer.y1));
        p.line_to((outer.x0, outer.y1));
        p.close_path();
        p.move_to((inner.x0, inner.y0));
        p.line_to((inner.x0, inner.y1));
        p.line_to((inner.x1, inner.y1));
        p.line_to((inner.x1, inner.y0));
        p.close_path();
        Some(p)
    }

    /// Glyph outline in font units, y-down (flipped), cached per (face, glyph). A `.notdef`
    /// without an outline is drawn as [`FontDb::missing_box`], so a missing character never
    /// disappears.
    pub fn outline(&self, face: &FontFace, gid: u32) -> Arc<BezPath> {
        let key = (face.id, gid);
        if let Some(p) = self.outlines.lock().unwrap_or_else(PoisonError::into_inner).get(&key) {
            return p.clone();
        }
        let drawn = font_outline(face, gid);
        let p = Arc::new(if gid == 0 && drawn.elements().is_empty() { self.missing_box(face).unwrap_or(drawn) } else { drawn });
        let mut cache = self.outlines.lock().unwrap_or_else(PoisonError::into_inner);
        if cache.len() >= OUTLINE_CACHE_MAX {
            cache.clear();
        }
        cache.insert(key, p.clone());
        p
    }
}
