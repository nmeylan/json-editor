use egui::{Button, Response,  Ui};
use crate::components::icon;

pub fn icon(name: &'static str) -> egui::RichText {
    egui::RichText::new(name)
        .family(egui::FontFamily::Name("fa".into()))
        .size(12.0)
}

pub fn button(ui: &mut Ui, name: &'static str) -> Response {
    let button = Button::new(icon::icon(name));
    let response = ui.add(button);
    response
}