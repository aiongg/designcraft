//! Design tokens (InDesign's four interface brightness levels), fonts and egui style.

use std::sync::OnceLock;

use craft_ui::direction;
use craft_ui::fonts::{CraftFace, Family, I18nScripts, Stack, SystemFonts, UiFonts};
use egui::{Color32, CornerRadius, FontData, FontFamily, FontId, Stroke, TextStyle, Visuals};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Brightness {
    Dark,
    #[default]
    MediumDark,
    MediumLight,
    Light,
    /// Accessibility: black chrome, white text, yellow selection and focus.
    HighContrast,
}

impl Brightness {
    pub const ALL: [Brightness; 5] = [Brightness::Dark, Brightness::MediumDark, Brightness::MediumLight, Brightness::Light, Brightness::HighContrast];
    pub fn label(self) -> &'static str {
        match self {
            Brightness::Dark => "Dark",
            Brightness::MediumDark => "Medium Dark",
            Brightness::MediumLight => "Medium Light",
            Brightness::Light => "Light",
            Brightness::HighContrast => "High Contrast",
        }
    }
    pub fn id(self) -> &'static str {
        match self {
            Brightness::Dark => "dark",
            Brightness::MediumDark => "mediumDark",
            Brightness::MediumLight => "mediumLight",
            Brightness::Light => "light",
            Brightness::HighContrast => "highContrast",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|b| b.id().eq_ignore_ascii_case(s) || b.label().eq_ignore_ascii_case(s))
    }
}

/// Every colour the UI uses. Widgets never hard-code colours.
#[derive(Clone, Copy, Debug)]
pub struct Tokens {
    pub dark: bool,
    pub app_bar: Color32,
    pub panel: Color32,
    pub panel_darker: Color32,
    pub input: Color32,
    pub input_border: Color32,
    pub divider: Color32,
    pub text: Color32,
    pub text_strong: Color32,
    pub text_dim: Color32,
    pub text_disabled: Color32,
    pub icon: Color32,
    pub hover: Color32,
    pub tool_active: Color32,
    pub accent: Color32,
    pub accent_strong: Color32,
    pub row_selected: Color32,
    pub pasteboard: Color32,
    pub ruler: Color32,
    pub ruler_tick: Color32,
    pub tab_inactive: Color32,
    pub button: Color32,
    pub radius: u8,
    /// 1 pt chrome edges.
    pub border: Color32,
    /// Outline of fields and buttons.
    pub field_border: Color32,
    /// Active tool / toggle well.
    pub well: Color32,
    pub well_rim: Color32,
    pub ruler_text: Color32,
    pub section_divider: Color32,
    /// Panel tab strips, doc-tab bar, dock headers.
    pub tab_strip: Color32,
    /// Smart guides and spacing marks drawn over the page.
    pub smart_guide: Color32,
    /// Measurement labels on the canvas (smart dimensions, gaps).
    pub measure_bg: Color32,
    pub measure_text: Color32,
    /// Live-corner widget and corner diamonds.
    pub live_corner: Color32,
    /// The Windows caption Close button when hovered, and its glyph.
    pub caption_close: Color32,
    pub caption_close_text: Color32,
}

fn hex(s: u32) -> Color32 {
    Color32::from_rgb((s >> 16) as u8, (s >> 8) as u8, s as u8)
}

impl Tokens {
    pub fn for_brightness(b: Brightness) -> Self {
        let dark = Tokens {
            dark: true,
            app_bar: hex(0x262626),
            panel: hex(0x323232),
            panel_darker: hex(0x282828),
            input: hex(0x1f1f1f),
            input_border: hex(0x4a4a4a),
            divider: hex(0x1e1e1e),
            text: hex(0xd8d8d8),
            text_strong: hex(0xf2f2f2),
            text_dim: hex(0xa3a3a3),
            text_disabled: hex(0x6a6a6a),
            icon: hex(0xc8c8c8),
            hover: hex(0x424242),
            tool_active: hex(0x1b1b1b),
            accent: hex(0x378ef0),
            accent_strong: hex(0x1473e6),
            row_selected: hex(0x2d4f7c),
            pasteboard: hex(0x232323),
            ruler: hex(0x2e2e2e),
            ruler_tick: hex(0x9a9a9a),
            tab_inactive: hex(0x262626),
            button: hex(0x404040),
            radius: 1,
            border: hex(0x1e1e1e),
            field_border: hex(0x5a5a5a),
            well: hex(0x1b1b1b),
            well_rim: hex(0x3a3a3a),
            ruler_text: hex(0xd8d8d8),
            section_divider: hex(0x282828),
            tab_strip: hex(0x282828),
            smart_guide: hex(0x00c853),
            measure_bg: Color32::from_rgba_unmultiplied(70, 70, 70, 230),
            measure_text: Color32::WHITE,
            live_corner: hex(0xffe500),
            caption_close: hex(0xc42b1c),
            caption_close_text: Color32::WHITE,
        };
        match b {
            Brightness::Dark => dark,
            // Measured from InDesign 2026 (plan/indesign/11-observed-ui.md §1).
            Brightness::MediumDark => Tokens {
                app_bar: hex(0x535353),
                panel: hex(0x535353),
                panel_darker: hex(0x424242),
                input: hex(0x454545),
                input_border: hex(0x747474),
                divider: hex(0x4b4b4b),
                text: hex(0xffffff),
                text_strong: hex(0xffffff),
                text_dim: hex(0xb0b0b0),
                text_disabled: hex(0x8a8a8a),
                icon: hex(0xc2c2c2),
                hover: hex(0x606060),
                tool_active: hex(0x303030),
                row_selected: hex(0x4a6a92),
                pasteboard: hex(0x5e5e5e),
                ruler: hex(0x1f1f1f),
                ruler_tick: hex(0x858585),
                tab_inactive: hex(0x424242),
                button: hex(0x535353),
                border: hex(0x383838),
                field_border: hex(0x747474),
                well: hex(0x303030),
                well_rim: hex(0x565656),
                ruler_text: hex(0xffffff),
                section_divider: hex(0x4b4b4b),
                tab_strip: hex(0x424242),
                ..dark
            },
            Brightness::MediumLight => Tokens {
                dark: false,
                app_bar: hex(0xa8a8a8),
                panel: hex(0xb8b8b8),
                panel_darker: hex(0xaaaaaa),
                input: hex(0xd6d6d6),
                input_border: hex(0x8c8c8c),
                divider: hex(0x9c9c9c),
                text: hex(0x1e1e1e),
                text_strong: hex(0x000000),
                text_dim: hex(0x3c3c3c),
                text_disabled: hex(0x7a7a7a),
                icon: hex(0x2a2a2a),
                hover: hex(0xc8c8c8),
                tool_active: hex(0x9a9a9a),
                row_selected: hex(0x8eb2e0),
                pasteboard: hex(0xa0a0a0),
                ruler: hex(0xc0c0c0),
                ruler_tick: hex(0x3a3a3a),
                tab_inactive: hex(0xa6a6a6),
                button: hex(0xb8b8b8),
                border: hex(0x8c8c8c),
                field_border: hex(0x7a7a7a),
                well: hex(0x9a9a9a),
                well_rim: hex(0x8a8a8a),
                ruler_text: hex(0x1e1e1e),
                section_divider: hex(0x9c9c9c),
                tab_strip: hex(0xa6a6a6),
                ..dark
            },
            Brightness::Light => Tokens {
                dark: false,
                app_bar: hex(0xe4e4e4),
                panel: hex(0xf0f0f0),
                panel_darker: hex(0xe2e2e2),
                input: hex(0xffffff),
                input_border: hex(0xb4b4b4),
                divider: hex(0xd0d0d0),
                text: hex(0x1e1e1e),
                text_strong: hex(0x000000),
                text_dim: hex(0x555555),
                text_disabled: hex(0x9a9a9a),
                icon: hex(0x2c2c2c),
                hover: hex(0xdddddd),
                tool_active: hex(0xcfcfcf),
                row_selected: hex(0xc4dbf7),
                pasteboard: hex(0xd4d4d4),
                ruler: hex(0xf2f2f2),
                ruler_tick: hex(0x4a4a4a),
                tab_inactive: hex(0xdcdcdc),
                button: hex(0xf0f0f0),
                border: hex(0xc4c4c4),
                field_border: hex(0xa8a8a8),
                well: hex(0xcfcfcf),
                well_rim: hex(0xbdbdbd),
                ruler_text: hex(0x1e1e1e),
                section_divider: hex(0xd0d0d0),
                tab_strip: hex(0xe2e2e2),
                ..dark
            },
            Brightness::HighContrast => Tokens {
                app_bar: hex(0x000000),
                panel: hex(0x000000),
                panel_darker: hex(0x000000),
                input: hex(0x000000),
                input_border: hex(0xffffff),
                divider: hex(0xffffff),
                text: hex(0xffffff),
                text_strong: hex(0xffffff),
                text_dim: hex(0xe6e6e6),
                text_disabled: hex(0xa0a0a0),
                icon: hex(0xffffff),
                hover: hex(0x3d3d00),
                tool_active: hex(0x5c5c00),
                accent: hex(0xffeb3b),
                accent_strong: hex(0xffd600),
                row_selected: hex(0x5c5c00),
                pasteboard: hex(0x1a1a1a),
                ruler: hex(0x000000),
                ruler_tick: hex(0xffffff),
                tab_inactive: hex(0x000000),
                button: hex(0x000000),
                border: hex(0xffffff),
                field_border: hex(0xffffff),
                well: hex(0x5c5c00),
                well_rim: hex(0xffeb3b),
                ruler_text: hex(0xffffff),
                section_divider: hex(0xffffff),
                tab_strip: hex(0x000000),
                ..dark
            },
        }
    }
}

/// The interface language's tag as craft-i18n spells it (English is `""` in [`crate::i18n`]).
fn language_tag(lang: &str) -> &str {
    if lang.is_empty() { "en" } else { lang }
}

/// The craft-fonts faces this build embedded ([`designcraft_fonts::CRAFT_FONTS`]; empty unless
/// built with `CRAFT_FONTS_DIR`).
pub(crate) fn craft_faces() -> &'static [CraftFace] {
    static FACES: OnceLock<Vec<CraftFace>> = OnceLock::new();
    FACES.get_or_init(|| {
        designcraft_fonts::CRAFT_FONTS
            .iter()
            .map(|f| CraftFace { family: f.family, style: f.style, scripts: f.scripts, bytes: f.bytes, index: 0 })
            .collect()
    })
}

/// The UI faces every family starts with: Source Sans 3 (the semibold shares its line metrics)
/// and JetBrains Mono.
const UI_FONT: &[u8] = include_bytes!("../../../assets/fonts/SourceSans3-Regular.ttf");
const UI_SEMIBOLD: &[u8] = include_bytes!("../../../assets/fonts/SourceSans3-Semibold.ttf");
const MONO_FONT: &[u8] = include_bytes!("../../../assets/fonts/JetBrainsMono-Regular.ttf");

/// The UI font stack for the interface language `lang`: DesignCraft's faces, then the craft-fonts
/// faces and installed fonts in the suite's order ([`craft_ui::fonts`]).
pub(crate) fn font_stack(lang: &str, system: SystemFonts) -> Stack {
    font_stack_with(lang, system, craft_faces())
}

/// [`font_stack`] with the craft-fonts faces `craft`.
fn font_stack_with(lang: &str, system: SystemFonts, craft: &[CraftFace]) -> Stack {
    let languages: Vec<&str> = crate::i18n::LANGUAGES.iter().map(|(code, _)| language_tag(code)).collect();
    UiFonts::new(&I18nScripts)
        .app_face("ui", FontData::from_static(UI_FONT), &[FontFamily::Proportional])
        .app_face("ui-semibold", FontData::from_static(UI_SEMIBOLD), &[FontFamily::Name("semibold".into())])
        .app_face("mono", FontData::from_static(MONO_FONT), &[FontFamily::Monospace])
        .family(Family::named("semibold").weight(600))
        .craft_faces(craft)
        .languages(&languages)
        .ui_language(language_tag(lang))
        .system_fonts(system)
        .stack()
}

/// Install the UI fonts and the interface direction for the interface language `lang`. Call it
/// again when the language changes. Installed fonts are found in the background and join the
/// stack when their index is ready.
pub fn install_fonts(ctx: &egui::Context, lang: &str) {
    font_stack(lang, SystemFonts::AfterCraft).install(ctx);
    direction::apply(ctx, direction::resolve(language_tag(lang), direction::Setting::Language));
}

pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("semibold".into()))
}

pub fn apply(ctx: &egui::Context, t: &Tokens) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::NULL, *t));
    let text_styles: std::collections::BTreeMap<TextStyle, FontId> = [
        (TextStyle::Small, FontId::proportional(10.0)),
        (TextStyle::Body, FontId::proportional(11.0)),
        (TextStyle::Button, FontId::proportional(11.0)),
        (TextStyle::Heading, FontId::new(15.0, FontFamily::Name("semibold".into()))),
        (TextStyle::Monospace, FontId::monospace(11.5)),
    ]
    .into();
    let mut v = if t.dark { Visuals::dark() } else { Visuals::light() };
    let r = CornerRadius::same(t.radius);
    v.panel_fill = t.panel;
    v.window_fill = t.panel;
    v.extreme_bg_color = t.input;
    v.faint_bg_color = t.panel_darker;
    v.window_stroke = Stroke::new(1.0, t.divider);
    v.window_corner_radius = CornerRadius::same(4);
    v.menu_corner_radius = CornerRadius::same(4);
    v.selection.bg_fill = t.accent_strong;
    v.selection.stroke = Stroke::new(1.0, t.text_strong);
    v.hyperlink_color = t.accent;
    v.override_text_color = Some(t.text);
    craft_ui::theme::WidgetPalette { noninteractive: t.panel, inactive: t.button, hovered: t.hover, active: t.tool_active, open: t.hover }.apply_to(
        &mut v.widgets,
        r,
        Stroke::new(1.0, t.text),
    );
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, t.divider);
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, t.field_border);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, t.field_border);
    v.widgets.active.bg_stroke = Stroke::new(1.0, t.field_border);
    // InDesign buttons are outline-only.
    v.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    v.widgets.inactive.bg_fill = t.input;
    v.popup_shadow = egui::epaint::Shadow { offset: [0, 4], blur: 12, spread: 0, color: Color32::from_black_alpha(90) };
    ctx.set_visuals(v);
    ctx.global_style_mut(|s| {
        s.text_styles = text_styles.clone();
        s.spacing.item_spacing = egui::vec2(6.0, 4.0);
        s.spacing.button_padding = egui::vec2(6.0, 2.0);
        s.spacing.interact_size = egui::vec2(20.0, 21.0);
        s.spacing.menu_margin = egui::Margin::same(4);
        s.animation_time = 0.06;
    });
}

impl Tokens {
    /// Tokens stored by [`apply`].
    pub fn get(ctx: &egui::Context) -> Tokens {
        ctx.data(|d| d.get_temp::<Tokens>(egui::Id::NULL)).unwrap_or_else(|| Tokens::for_brightness(Brightness::Dark))
    }
}

#[cfg(test)]
mod font_tests {
    use super::*;

    /// A Latin interface keeps DesignCraft's faces first in every family, with or without
    /// craft-fonts.
    #[test]
    fn latin_interfaces_lead_with_the_app_faces() {
        let stack = font_stack("", SystemFonts::Off);
        for (family, first) in
            [(FontFamily::Proportional, "ui"), (FontFamily::Monospace, "mono"), (FontFamily::Name("semibold".into()), "ui-semibold")]
        {
            let chain = stack.chain(&family);
            assert_eq!(chain.first().map(String::as_str), Some(first), "{family:?}: {chain:?}");
        }
    }

    /// Japanese takes BIZ UDPGothic, craft-fonts' UI face, ahead of its Mincho faces.
    #[test]
    fn japanese_interface_uses_the_gothic_ui_face() {
        if craft_faces().iter().all(|f| !f.scripts.contains(&"Jpan")) {
            eprintln!("skipped: built without craft-fonts (set CRAFT_FONTS_DIR to a checkout)");
            return;
        }
        let stack = font_stack("ja", SystemFonts::Off);
        let chain = stack.chain(&FontFamily::Proportional);
        let japanese: Vec<&str> = craft_faces().iter().filter(|f| f.scripts.contains(&"Jpan")).map(|f| f.family).collect();
        let first_craft = chain.iter().find(|n| japanese.iter().any(|family| n.contains(family)));
        assert!(first_craft.is_some_and(|n| n.contains("BIZ UDPGothic")), "{chain:?}");
    }

    /// Every shipped translation and every language's menu name draws with the UI fonts alone, no
    /// installed fonts. Latin and Cyrillic need only the bundled faces; Japanese, Chinese, Arabic
    /// and Georgian are checked in a build whose craft-fonts (`CRAFT_FONTS_DIR`) has a face for
    /// their script, and are left to installed fonts otherwise.
    #[test]
    fn ui_fonts_cover_every_catalog() {
        use craft_ui::fonts::{Sample, assert_covers};
        let craft_has = |script: &str| craft_faces().iter().any(|f| f.scripts.contains(&script));
        let checked = |lang: &str| match lang {
            "ja" => craft_has("Jpan"),
            "zh" => craft_has("Hans"),
            "ar" => craft_has("Arab"),
            "ka" => craft_has("Geor"),
            _ => true,
        };
        let entries: Vec<(&str, &str, &str)> = crate::i18n::catalog_entries().into_iter().filter(|(lang, ..)| checked(lang)).collect();
        let samples = entries.iter().map(|(lang, key, text)| Sample { language: language_tag(lang), key, text });
        assert_covers(&font_stack("", SystemFonts::Off), samples);
    }

    /// A craft face for scripts no interface language uses (Hebrew, Devanagari) draws in every UI
    /// family, so a tab in such a script never meets a face-less glyph.
    #[test]
    fn additional_craft_scripts_render_in_every_ui_family() {
        let bytes: &'static [u8] = Box::leak(designcraft_fonts::testing::font_with("DC extra scripts", &['ד', 'क']).unwrap().into_boxed_slice());
        let face = CraftFace { family: "DC extra scripts", style: "Regular", scripts: &["Hebr", "Deva"], bytes, index: 0 };
        let ctx = egui::Context::default();
        font_stack_with("", SystemFonts::Off, &[face]).install(&ctx);
        let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
        output.textures_delta.clear();
        ctx.fonts_mut(|fonts| {
            for family in [FontFamily::Proportional, FontFamily::Monospace, FontFamily::Name("semibold".into())] {
                let font = FontId::new(13.0, family);
                assert!(fonts.has_glyph(&font, 'ד') && fonts.has_glyph(&font, 'क'), "missing supplied script in {font:?}");
                let galley = fonts.layout_no_wrap("דक".into(), font, Color32::WHITE);
                assert!(galley.size().x > 0.0 && galley.rect.is_finite());
            }
        });
    }

    /// Installing works with whatever faces this build has.
    #[test]
    fn install_fonts_works_without_craft_fonts() {
        let ctx = egui::Context::default();
        install_fonts(&ctx, "ja");
        let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
        output.textures_delta.clear();
        ctx.fonts_mut(|fonts| assert!(fonts.has_glyphs(&FontId::new(13.0, FontFamily::Proportional), "DesignCraft")));
    }
}
