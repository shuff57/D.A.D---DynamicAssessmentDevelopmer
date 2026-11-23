//! Native text and controls for the in-app student preview. The browser still grades the answers.

use egui::{Ui, WidgetInfo, WidgetType};
use serde::Deserialize;

use super::livecdp::LiveTab;

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Item {
    Text { text: String },
    Field { id: usize, label: String, hint: String, value: String, multiline: bool },
    Choice { id: usize, label: String, group: String, checked: bool },
    Select { id: usize, label: String, value: String, options: Vec<(String, String)> },
    Button { id: usize, label: String },
    VideoLink { url: String },
}

pub struct StudentText {
    items: Vec<Item>,
    focus: Option<usize>,
}

pub struct StudentAction {
    id: Option<usize>,
    edits: Vec<serde_json::Value>,
}

impl StudentText {
    pub fn read(tab: &LiveTab, preview_url: &str, action: Option<StudentAction>) -> Result<Self, String> {
        let (id, edits) = action.map_or((None, Vec::new()), |action| (action.id, action.edits));
        let args = serde_json::json!([preview_url, edits, id]);
        let value = tab.eval(&format!("({})(...{args})", include_str!("student_text.js")))?;
        let items: Vec<Item> = serde_json::from_value(value).map_err(|error| format!("Could not read student preview text: {error}"))?;
        if items.is_empty() { return Err("The student player is still loading. Refresh the text view to try again.".into()); }
        let focus = id.and_then(|_| {
            items.iter().position(|item| matches!(item, Item::Text { text } if text.contains("Score on last try")))
                .or_else(|| items.iter().position(|item| matches!(item, Item::Text { .. })))
        });
        Ok(Self { items, focus })
    }

    pub fn draw(&mut self, ui: &mut Ui, working: bool) -> Option<StudentAction> {
        let mut radio = None;
        let mut action = None;
        egui::ScrollArea::vertical().id_salt("student_text").show(ui, |ui| {
            ui.add_enabled_ui(!working, |ui| {
                for (index, item) in self.items.iter_mut().enumerate() {
                    ui.push_id(index, |ui| match item {
                        Item::Text { text } => {
                            let caption = ui.add(egui::Label::new(crate::theme::text(&*text))
                                .sense(egui::Sense::focusable_noninteractive()).wrap());
                            if self.focus == Some(index) {
                                caption.request_focus();
                                caption.scroll_to_me(Some(egui::Align::Center));
                                self.focus = None;
                            }
                        }
                        Item::Field { label, hint, value, multiline, .. } => {
                            let caption = ui.label(crate::theme::strong(&*label));
                            if *multiline {
                                ui.add(egui::TextEdit::multiline(value).hint_text(&*hint)).labelled_by(caption.id);
                            } else {
                                ui.add(egui::TextEdit::singleline(value).hint_text(&*hint)).labelled_by(caption.id);
                            }
                        }
                        Item::Choice { id, label, group, checked } if !group.is_empty() => {
                            if ui.radio(*checked, &*label).clicked() {
                                radio = Some((*id, group.clone()));
                            }
                        }
                        Item::Choice { label, checked, .. } => { ui.checkbox(checked, &*label); }
                        Item::Select { label, value, options, .. } => {
                            let caption = ui.label(crate::theme::strong(&*label));
                            let selected = options.iter().find(|(key, _)| key == value)
                                .map_or("Choose an answer", |(_, text)| text.as_str());
                            egui::ComboBox::from_id_salt("answer").selected_text(selected).show_ui(ui, |ui| {
                                for (key, text) in options { ui.selectable_value(value, key.clone(), &*text); }
                            }).response.labelled_by(caption.id);
                        }
                        Item::Button { id, label } => {
                            let button = crate::pageshell::outline(ui, label);
                            button.widget_info(|| WidgetInfo::labeled(WidgetType::Button, !working, &*label));
                            if button.clicked() { action = Some(*id); }
                        }
                        Item::VideoLink { url } => {
                            if let Ok(url) = super::question_help::video_url(url) {
                                ui.hyperlink_to("Watch the help video", url.as_str());
                            }
                        }
                    });
                }
            });
        });
        if let Some((chosen, group)) = radio {
            for item in &mut self.items {
                if let Item::Choice { id, group: item_group, checked, .. } = item && *item_group == group {
                    *checked = *id == chosen;
                }
            }
        }
        action.map(|id| self.action(Some(id)))
    }

    pub fn sync(&self) -> StudentAction { self.action(None) }

    fn action(&self, id: Option<usize>) -> StudentAction {
        let edits = self.items.iter().filter_map(|item| {
            let (id, value) = match item {
                Item::Field { id, value, .. } | Item::Select { id, value, .. } => (*id, value.clone()),
                Item::Choice { id, checked, .. } => (*id, checked.to_string()),
                _ => return None,
            };
            Some(serde_json::json!({ "id": id, "value": value }))
        }).collect();
        StudentAction { id, edits }
    }
}
