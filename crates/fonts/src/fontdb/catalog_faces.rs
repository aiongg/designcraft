//! Faces made from a font catalog's fonts, for an app that shapes and draws with its own face
//! objects (skrifa, harfrust): each face of a font file is made once, kept while its file is
//! unchanged and in the catalog, and let go when a rescan finds the file changed or removed.
//!
//! Nothing here knows the app's face type `F`, so any app on `craft-font-catalog` can use it.

use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use std::time::SystemTime;

use craft_font_catalog::{Catalog, FaceInfo, FontData, FontFileError};

/// The largest font file read again here (the catalog's own limit for a load).
const MAX_FILE: u64 = 512 << 20;

/// Which face of which font data a cached face was made from.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum FaceKey {
    /// A face of a font file: the file and the face's number in it ([`FaceInfo::index`]).
    File(PathBuf, u32),
    /// A face of font data added to the catalog as bytes: the data's address (the cache holds the
    /// data, so the address isn't reused while the face is cached) and the face's number.
    Added(usize, u32),
}

/// A file's length and modification time.
type Stamp = (u64, Option<SystemTime>);

fn stamp(path: &Path) -> Option<Stamp> {
    std::fs::metadata(path).ok().map(|m| (m.len(), m.modified().ok()))
}

struct Cached<F> {
    face: Arc<F>,
    info: FaceInfo,
    bytes: Arc<[u8]>,
    /// The file's stamp before it was read, so a change during the read shows at the next
    /// refresh.
    stamp: Option<Stamp>,
}

struct State<F> {
    /// The catalog generation the cached faces were last checked at.
    generation: Option<u64>,
    faces: HashMap<FaceKey, Cached<F>>,
    /// Files whose faces were let go because the file changed: the data the catalog had for them
    /// (it hands a file's data out again while anything holds it), and the data read since.
    stale: HashMap<PathBuf, (Weak<[u8]>, Option<Weak<[u8]>>)>,
}

/// The app's faces (`F`) made from a [`Catalog`]'s fonts, by font file and face index.
pub struct FaceCache<F> {
    state: Mutex<State<F>>,
}

impl<F> Default for FaceCache<F> {
    fn default() -> Self {
        Self { state: Mutex::new(State { generation: None, faces: HashMap::new(), stale: HashMap::new() }) }
    }
}

impl<F> FaceCache<F> {
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> MutexGuard<'_, State<F>> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The face made from `info`'s data: the one made before for the same file and face index,
    /// else `make` (called without locks held) from the data `catalog` loads. `make` gets the
    /// data's own index ([`FontData::index`], which differs from [`FaceInfo::index`] for a Mac
    /// suitcase font); `None` from it is [`FontFileError::NotAFont`]. Two threads making the same
    /// face at once both make it, and both get the first one cached.
    pub fn get_or_make(
        &self,
        catalog: &Catalog,
        info: &FaceInfo,
        make: impl FnOnce(&FaceInfo, &FontData) -> Option<F>,
    ) -> Result<Arc<F>, FontFileError> {
        if let Some(path) = &info.path
            && let Some(c) = self.lock().faces.get(&FaceKey::File(path.clone(), info.index))
        {
            return Ok(Arc::clone(&c.face));
        }
        let stamp = info.path.as_deref().and_then(stamp);
        let data = self.load(catalog, info)?;
        let key = match &info.path {
            Some(path) => FaceKey::File(path.clone(), info.index),
            None => FaceKey::Added(Arc::as_ptr(&data.bytes).cast::<u8>() as usize, info.index),
        };
        if let Some(c) = self.lock().faces.get(&key) {
            return Ok(Arc::clone(&c.face));
        }
        let face = Arc::new(make(info, &data).ok_or(FontFileError::NotAFont)?);
        let mut state = self.lock();
        let cached = state.faces.entry(key).or_insert(Cached { face, info: info.clone(), bytes: data.bytes, stamp });
        Ok(Arc::clone(&cached.face))
    }

    /// `info`'s data from `catalog`, or read from its file again when the catalog hands out the
    /// data of a file that changed since.
    fn load(&self, catalog: &Catalog, info: &FaceInfo) -> Result<FontData, FontFileError> {
        let data = catalog.load(info)?;
        let Some(path) = &info.path else { return Ok(data) };
        let fresh = {
            let state = self.lock();
            let Some((old, fresh)) = state.stale.get(path) else { return Ok(data) };
            if !old.upgrade().is_some_and(|old| Arc::ptr_eq(&old, &data.bytes)) {
                return Ok(data);
            }
            fresh.as_ref().and_then(Weak::upgrade)
        };
        let bytes = match fresh {
            Some(bytes) => bytes,
            None => {
                let bytes: Arc<[u8]> = read_file(path)?.into();
                if let Some(entry) = self.lock().stale.get_mut(path) {
                    entry.1 = Some(Arc::downgrade(&bytes));
                }
                bytes
            }
        };
        Ok(FontData { bytes, index: data.index })
    }

    /// Let go of the faces whose file changed or left `catalog`, or whose added data was removed
    /// from it, since the last refresh, and return them (for the app to drop what it made from
    /// them). Nothing to do while the catalog's generation is the one last checked.
    pub fn refresh(&self, catalog: &Catalog) -> Vec<(FaceKey, Arc<F>)> {
        let generation = catalog.generation();
        if self.lock().generation == Some(generation) {
            return Vec::new();
        }
        let listed: HashSet<(PathBuf, u32)> = catalog.faces().iter().filter_map(|f| Some((f.path.clone()?, f.index))).collect();
        let mut state = self.lock();
        state.generation = Some(generation);
        let mut gone = Vec::new();
        for (key, c) in std::mem::take(&mut state.faces) {
            let keep = match &key {
                FaceKey::File(path, index) => listed.contains(&(path.clone(), *index)) && stamp(path) == c.stamp,
                FaceKey::Added(..) => catalog.load(&c.info).is_ok_and(|d| Arc::ptr_eq(&d.bytes, &c.bytes)),
            };
            if keep {
                state.faces.insert(key, c);
                continue;
            }
            if let FaceKey::File(path, _) = &key {
                state.stale.insert(path.clone(), (Arc::downgrade(&c.bytes), None));
            }
            gone.push((key, c.face));
        }
        state.stale.retain(|_, (old, _)| old.strong_count() > 0);
        gone
    }
}

fn read_file(path: &Path) -> Result<Vec<u8>, FontFileError> {
    let file = std::fs::File::open(path)?;
    let meta = file.metadata()?;
    if !meta.is_file() {
        return Err(FontFileError::NotAFont);
    }
    if meta.len() > MAX_FILE {
        return Err(FontFileError::TooLarge { size: meta.len(), max: MAX_FILE });
    }
    let mut data = Vec::new();
    file.take(MAX_FILE).read_to_end(&mut data)?;
    Ok(data)
}
