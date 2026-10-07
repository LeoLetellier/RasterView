pub(crate) mod buttons;

use egui::{FontData, FontDefinitions, FontFamily};
use egui_phosphor as icon;

const REGULAR_FONT: &str = "phosphor-regular-font";
const FILL_FONT: &str = "phosphor-fill-font";
const REGULAR_FAMILY: &str = "phosphor-regular";
const FILL_FAMILY: &str = "phosphor-fill";

fn regular_family() -> FontFamily {
    FontFamily::Name(REGULAR_FAMILY.into())
}
fn fill_family() -> FontFamily {
    FontFamily::Name(FILL_FAMILY.into())
}

/// Call once in `setup_custom_fonts`, AFTER your text fonts are in
/// `FontDefinitions` (it copies the current Proportional list as fallback).
/// Replaces both `icon::add_to_fonts(...)` calls.
pub fn install_phosphor(fonts: &mut FontDefinitions) {
    fonts.font_data.insert(
        REGULAR_FONT.to_owned(),
        icon::Variant::Regular.font_data().into(),
    );
    fonts
        .font_data
        .insert(FILL_FONT.to_owned(), icon::Variant::Fill.font_data().into());

    // Text fonts as fallback: silences the '◻' / '?' warning
    let fallback = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();

    let named = |first: &str| {
        let mut v = vec![first.to_owned()];
        v.extend(fallback.iter().cloned());
        v
    };
    fonts.families.insert(regular_family(), named(REGULAR_FONT));
    fonts.families.insert(fill_family(), named(FILL_FONT));

    // Keep Regular glyphs working for ui.button(icon::regular::X) elsewhere
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .push(REGULAR_FONT.to_owned());
}
