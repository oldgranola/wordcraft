//! View › Markdown Source: the text pane that takes the page's place. The text itself and what it
//! means for the document are the engine's (`wordcraft_engine::cmd::md_source`).

use egui::text::{CCursor, CCursorRange};
use egui::{FontId, Key, KeyboardShortcut, Modifiers, Ui};
use serde_json::json;

use crate::WordApp;
use crate::clock::now_ms;
use crate::theme::Tokens;

/// How often a text with edits reaches the document while it is being typed, in milliseconds, so
/// crash recovery has them.
const APPLY_EVERY_MS: f64 = 5000.0;

/// Draw the pane in the place of the page.
pub fn show(app: &mut WordApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    // The page isn't drawn, so it can't have the keyboard.
    app.canvas.focused = false;
    app.canvas.want_focus = false;
    // A focused text box turns the global shortcuts off.
    if ui.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::S))) {
        let _ = app.run("file.save", json!({}));
    }
    let now = now_ms();
    if app.session.md_source.as_ref().is_some_and(|src| src.is_pending() && now - src.applied_at_ms > APPLY_EVERY_MS) {
        wordcraft_engine::cmd::md_source::apply(&mut app.session);
        if let Some(src) = app.session.md_source.as_mut() {
            src.applied_at_ms = now;
        }
    }
    let id = ui.scope_id().with(("md_source", app.session.document_id()));
    let Some(src) = app.session.md_source.as_mut() else { return };
    if src.loses_content {
        egui::Frame::NONE.fill(t.hover).inner_margin(egui::Margin::symmetric(12, 6)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(tl!("This document has content Markdown can't hold. Changing the text here removes it; one Undo afterwards brings it back."));
        });
    }
    let mut changed = false;
    egui::ScrollArea::vertical().id_salt(id).auto_shrink(false).show(ui, |ui| {
        let size = ui.available_size();
        let mut out = egui::TextEdit::multiline(&mut src.text)
            .id(id)
            .hint_text(tl!("Markdown Source"))
            .font(FontId::monospace(14.0))
            .desired_width(f32::INFINITY)
            .min_size(size)
            .frame(egui::Frame::NONE.inner_margin(16))
            .text_color(t.text)
            .lock_focus(true)
            .show(ui);
        changed = out.response.changed();
        if src.place_caret {
            src.place_caret = false;
            let caret = CCursor::new(src.caret);
            out.state.cursor.set_char_range(Some(CCursorRange::one(caret)));
            out.state.store(ui.ctx(), id);
            out.response.request_focus();
            let spot = out.galley.pos_from_cursor(caret).translate(out.galley_pos.to_vec2());
            ui.scroll_to_rect(spot, Some(egui::Align::Center));
        } else if let Some(range) = out.cursor_range {
            src.caret = range.primary.index.into();
        }
    });
    if changed {
        app.session.dirty = true;
        ui.ctx().request_repaint_after(std::time::Duration::from_secs_f64(APPLY_EVERY_MS / 1000.0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::kittest::Queryable;

    fn harness(a: WordApp) -> egui_kittest::Harness<'static, WordApp> {
        egui_kittest::Harness::builder().with_size(egui::vec2(1440.0, 900.0)).build_ui_state(
            |ui, app: &mut WordApp| {
                let ctx = ui.ctx().clone();
                app.logic(&ctx);
                app.ui(ui);
            },
            a,
        )
    }

    /// A saved Markdown file, with its folder for the test to remove.
    fn opened(name: &str) -> (WordApp, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("wordcraft-ui-md-source-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("note.md");
        std::fs::write(&path, "# Heading\n\nSome text.\n").unwrap();
        let mut a = WordApp::new(wordcraft_engine::Session::new(wordcraft_doc::Document::new()), crate::Services::default());
        a.run("file.open", json!({"path": path.to_string_lossy()})).unwrap();
        (a, dir)
    }

    #[test]
    fn the_pane_shows_the_text_and_takes_the_keyboard() {
        let (mut a, dir) = opened("keys");
        a.run("view.markdownSource", json!({})).unwrap();
        let mut h = harness(a);
        h.run();
        assert!(!h.state().canvas.focused, "the page isn't drawn");
        assert!(h.ctx.text_edit_focused(), "the text box has the keyboard");
        assert!(h.get_all_by_value("# Heading\n\nSome text.\n").next().is_some(), "the Markdown is shown");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn typing_is_an_unsaved_change_and_ctrl_s_saves_it() {
        let (mut a, dir) = opened("save");
        a.run("view.markdownSource", json!({})).unwrap();
        let mut h = harness(a);
        h.run();
        h.state_mut().session.md_source.as_mut().unwrap().text.insert_str(0, "Typed. ");
        h.state_mut().session.dirty = true;
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::S);
        h.run();
        let saved = std::fs::read_to_string(dir.join("note.md")).unwrap();
        assert!(saved.contains("Typed."), "{saved}");
        assert!(!h.state().session.dirty);
        assert!(h.state().session.md_source.is_some(), "saving keeps the pane");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_ribbon_greys_out_edits_while_the_pane_is_open() {
        let (mut a, dir) = opened("grey");
        assert_eq!(a.session.why_disabled("format.bold"), None);
        a.run("view.markdownSource", json!({})).unwrap();
        assert_eq!(a.session.why_disabled("format.bold"), Some("leave Markdown Source first"));
        assert_eq!(a.session.why_disabled("view.markdownSource"), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
