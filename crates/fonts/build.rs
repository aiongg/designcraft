//! Optional craft-fonts build input (https://github.com/storytold/craft-fonts). With
//! `CRAFT_FONTS_DIR=<checkout>` the document faces named here are embedded as `CRAFT_FONTS`;
//! unset, `CRAFT_FONTS` is empty. The interface embeds its own faces (`designcraft-ui-egui`'s
//! build). Web (wasm32) builds embed only BIZ UDPGothic Regular, to stay within the web size
//! budget. It only reads the local checkout: no network.

/// The document faces: the Japanese faces the font menus offer, Mincho first for document text.
const DOCUMENT_FAMILIES: &[&str] = &["Shippori Mincho", "BIZ UDMincho", "BIZ UDPGothic"];

fn main() {
    let wasm = craft_fonts_build::target_is_wasm();
    let embedded = craft_fonts_build::Embed::documents(DOCUMENT_FAMILIES)
        .filter(move |face| !wasm || (face.family == "BIZ UDPGothic" && face.style == "Regular"))
        .write("craft_fonts.rs");
    if let Err(e) = embedded {
        println!("cargo::error={e}");
    }
}
