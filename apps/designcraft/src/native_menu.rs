//! macOS: DesignCraft's menu tree as the system menu bar, through craft-menu, which lays it out the
//! Mac way (app menu, Window, Help) and drives AppKit. A clicked item runs through the same command
//! path as the in-window menus. Key equivalents come back to egui as the keys pressed, so
//! DesignCraft's own shortcut handling runs them, as on the other platforms; ⌘C, ⌘X and ⌘V arrive
//! as egui's copy, cut and paste events. Enabled and checked states follow the app after each
//! input and a few times per second.

use std::hash::{DefaultHasher, Hash, Hasher};

use craft_menu::{ItemRole, LayoutOptions, Menu, MenuBar, MenuRole, Node};
use designcraft_ui_egui::menus::{self, AppMenuItem, Item};
use designcraft_ui_egui::{DesignApp, cjk_features, i18n::tr, now_ms};
use serde_json::Value;

const APP: &str = "DesignCraft";

/// The submenu macOS shows as the app menu's Appearance.
const THEMES: &str = "Interface Color Theme";

pub struct NativeMenu {
    menu: craft_menu::NativeMenu,
}

impl NativeMenu {
    /// Install the menu bar: once, on the main thread, in eframe's creator. `None` when AppKit
    /// refused it or `DESIGNCRAFT_NO_NATIVE_MENU` is set; the window keeps its own menus then.
    pub fn install(app: &mut DesignApp, ctx: &egui::Context) -> Option<Self> {
        if std::env::var_os("DESIGNCRAFT_NO_NATIVE_MENU").is_some() {
            return None;
        }
        let menu = craft_menu::mac::install(ctx, &layout(app, ctx).bar)?;
        app.native_menu = true;
        Some(Self { menu })
    }

    /// Before egui reads the frame's input: key equivalents become the key presses they were.
    pub fn raw_input(&mut self, raw: &mut egui::RawInput) {
        self.menu.raw_input(raw);
    }

    /// Run the items clicked since the last frame, like in-window menu clicks.
    pub fn run_clicks(&mut self, app: &mut DesignApp, ctx: &egui::Context) {
        for id in self.menu.take_clicks(ctx) {
            match command(&id) {
                Some((cmd, params)) => menus::activate_native(app, ctx, cmd, &params),
                None => log::warn!("menu: no command for the item {id}"),
            }
        }
    }

    /// After the frame's UI: bring the menus up to date with the app.
    pub fn sync(&mut self, app: &DesignApp, ctx: &egui::Context) {
        for c in self.menu.sync_with(state_hash(app, ctx), || layout(app, ctx)) {
            log::info!("menu: {} is {} on macOS and {} in {APP}; {}", c.shortcut, c.system, c.command, c.resolution);
        }
    }
}

/// What the menus show depends on: the interface language, Show CJK Features (hidden commands),
/// and enabled and checked states. Input can change those, and so can work that ends without
/// input (a document loading, the control channel), so they are also refreshed every 250 ms.
fn state_hash(app: &DesignApp, ctx: &egui::Context) -> u64 {
    let mut h = DefaultHasher::new();
    app.ui.language.hash(&mut h);
    cjk_features(app).hash(&mut h);
    ctx.text_edit_focused().hash(&mut h);
    craft_menu::had_input(ctx).then_some(ctx.cumulative_frame_nr()).hash(&mut h);
    ((now_ms() / 250.0) as u64).hash(&mut h);
    h.finish()
}

fn layout(app: &DesignApp, ctx: &egui::Context) -> craft_menu::Layout {
    let lang = app.ui.language.as_str();
    let label = |english: &'static str| tr(lang, &english.replace("{app}", APP)).to_string();
    // The app's other shortcuts (canvas and panel keys) count when the layout resolves clashes
    // with the system's keys.
    let extra: Vec<(String, String)> = menus::effective_shortcuts(app).into_iter().map(|(id, sc)| (sc, id)).collect();
    let options = LayoutOptions { label: &label, ..LayoutOptions::new(APP).extra_shortcuts(&extra) };
    craft_menu::mac_layout(&menu_bar(app, ctx), &options)
}

/// DesignCraft's menus, tagged for the Mac layout. About, Settings and Quit come from
/// `MAC_APP_MENU`; its other items (Join the Discord) follow them in the app menu.
fn menu_bar(app: &DesignApp, ctx: &egui::Context) -> MenuBar {
    let lang = app.ui.language.as_str();
    let mut own = Vec::new();
    for entry in menus::MAC_APP_MENU {
        let AppMenuItem::Cmd { label, command, .. } = *entry else { continue };
        let it = item(app, ctx, command, &Value::Null, tr(lang, label), shortcut(command));
        own.push(Node::Item(match command {
            "help.about" => it.role(ItemRole::About),
            "app.preferences" => it.role(ItemRole::Settings),
            // ⌘Q is left to the layout, which reports it as a click: Quit then runs with its
            // unsaved-documents prompt also while a text field or dialog has the keys.
            "app.quit" => craft_menu::Item { shortcut: None, ..it }.role(ItemRole::Quit),
            _ => it,
        }));
    }
    let mut bar = MenuBar { menus: vec![Menu::new(APP, own).role(MenuRole::App)] };
    for (title, entries) in menus::native_menu_tree() {
        let role = match title {
            "File" => MenuRole::File,
            "Window" => MenuRole::Window,
            "Help" => MenuRole::Help,
            _ => MenuRole::Normal,
        };
        bar.menus.push(Menu::new(tr(lang, title), nodes(app, ctx, &entries)).role(role));
    }
    bar
}

fn nodes(app: &DesignApp, ctx: &egui::Context, entries: &[Item]) -> Vec<Node> {
    let lang = app.ui.language.as_str();
    let mut out = Vec::with_capacity(entries.len());
    for e in entries {
        match e {
            Item::Sep => out.push(Node::Separator),
            Item::Sub(label, children) => {
                let sub = Node::submenu(tr(lang, label), nodes(app, ctx, children));
                out.push(if label == THEMES { sub.role(ItemRole::Appearance) } else { sub });
            }
            // Hidden CJK commands; the menus follow when Show CJK Features changes.
            Item::Cmd { id, .. } if !menus::shown(app, id) => {}
            Item::Cmd { label, id, params, shortcut } => out.push(item(app, ctx, id, params, tr(lang, label), *shortcut).into()),
        }
    }
    out
}

/// One command item. A click reports only the item's id, so a command with parameters carries
/// them in it (see [`command`]); those items show no shortcut, since the shortcut runs the
/// command without them.
fn item(app: &DesignApp, ctx: &egui::Context, id: &str, params: &Value, label: &str, shortcut: Option<&str>) -> craft_menu::Item {
    let item_id = if params.is_null() { id.to_string() } else { format!("{id}?{params}") };
    let mut it = craft_menu::Item::new(item_id, label).enabled(menus::native_menu_enabled(app, ctx, id));
    if params.is_null()
        && let Some(sc) = shortcut
    {
        it = it.shortcut(sc);
    }
    if let Some(on) = menus::checked(app, id, params) {
        it = it.checked(on);
    }
    it
}

/// The command and parameters of a menu item id: `edit.copy`, or `window.panel?{"panel":"pages"}`.
fn command(item_id: &str) -> Option<(&str, Value)> {
    match item_id.split_once('?') {
        None => Some((item_id, Value::Null)),
        Some((id, params)) => serde_json::from_str(params).ok().map(|p| (id, p)),
    }
}

/// A command's default shortcut, as the in-window menus show it.
fn shortcut(id: &str) -> Option<&'static str> {
    menus::ui_label(id).and_then(|(_, sc)| sc).or_else(|| designcraft_engine::find_command(id).and_then(|c| c.shortcut))
}
