//! Settings form, confirmation dialog and the multi-step assistant.
use crate::{Msg, Shell};
use windows_reactor::*;

const AUDIO: [&str; 3] = ["System default", "Speakers", "Headphones"];
const SCALING: [&str; 3] = ["Fit window", "Integer scale", "Native 1:1"];
const KINDS: [&str; 3] = ["Octane2", "O2", "Origin 350"];
const CARDS: [(&str, &str); 3] = [("Octane2", "IP30, SI graphics"), ("O2", "IP32, CRIME"), ("Origin 350", "IP35")];

impl Shell {
    pub fn settings_page(&self, cx: &ViewContext<Self>) -> View {
        let theme_idx = match self.theme {
            WindowTheme::System => 0,
            WindowTheme::Light => 1,
            _ => 2,
        };
        let cards = CARDS.iter().enumerate().map(|(i, (name, sub))| {
            Button::new()
                .style(if self.card == i { ButtonStyle::Accent } else { ButtonStyle::Default })
                .automation_id(format!("card-{i}"))
                .on_click(cx.message(Msg::Card(i)))
                .content(Border::new().padding(Thickness::xy(8.0, 6.0)).content(StackPanel::new().children((
                    TextBlock::new().text(*name).font_weight(FontWeight::SEMI_BOLD),
                    TextBlock::new().text(*sub).font_size(12.0),
                ))))
                .into()
        });
        ScrollViewer::new()
            .content(
                StackPanel::new().margin(24.0).spacing(14.0).max_width(560.0).horizontal_alignment(HorizontalAlignment::Left).children((
                    TextBlock::new().text("Machine settings").font_size(24.0),
                    TextBox::new()
                        .automation_id("setting-name")
                        .header(TextBlock::new().text("Machine name"))
                        .placeholder_text("Name")
                        .text(&self.name)
                        .on_text_changed(cx.callback(Msg::Name)),
                    ToggleSwitch::new()
                        .automation_id("setting-grab")
                        .header(TextBlock::new().text("Grab input on click"))
                        .is_on(self.core.borrow().grabbed)
                        .on_toggled(cx.callback(Msg::Grab)),
                    ComboBox::new()
                        .automation_id("setting-audio")
                        .header(TextBlock::new().text("Audio output"))
                        .items_source(AUDIO)
                        .selected_index(self.audio)
                        .on_selection_changed(cx.callback(Msg::Audio)),
                    RadioButtons::new()
                        .automation_id("setting-scaling")
                        .header(TextBlock::new().text("Display scaling"))
                        .items_source(SCALING)
                        .selected_index(self.scaling)
                        .on_selection_changed(cx.callback(Msg::Scaling)),
                    RadioButtons::new()
                        .automation_id("setting-theme")
                        .header(TextBlock::new().text("Theme"))
                        .items_source(["System", "Light", "Dark"])
                        .selected_index(Some(theme_idx))
                        .on_selection_changed(cx.callback(|i: Option<usize>| {
                            Msg::SetTheme(match i {
                                Some(1) => WindowTheme::Light,
                                Some(2) => WindowTheme::Dark,
                                _ => WindowTheme::System,
                            })
                        })),
                    TextBlock::new().text("Machine type"),
                    StackPanel::new().orientation(Orientation::Horizontal).spacing(8.0).children(cards.collect::<Vec<View>>()),
                    StackPanel::new().orientation(Orientation::Horizontal).spacing(8.0).children((
                        Button::new().automation_id("btn-open").content("Choose disk image...").on_click(cx.message(Msg::PickFile)),
                        Button::new().automation_id("btn-assist").content("New machine assistant...").on_click(cx.message(Msg::AssistOpen)),
                    )),
                    TextBlock::new().automation_id("picked-file").text(format!("Disk image: {}", self.last_file)),
                )),
            )
            .content_dialog(self.assist_dialog(cx))
    }

    pub fn confirm_dialog(&self, cx: &ViewContext<Self>) -> ContentDialog {
        ContentDialog::new()
            .title("Reset the machine?")
            .primary_button_text("Reset")
            .close_button_text("Cancel")
            .default_button(ContentDialogButton::Close)
            .is_open(self.confirm_open)
            .on_closed(cx.callback(Msg::ConfirmClosed))
            .content("Unsaved guest state is lost.")
    }

    /// WinUI closes a ContentDialog on every button, so a multi-step flow closes it and a short
    /// timer reopens it with the next step (see assist_closed).
    pub fn assist_dialog(&self, cx: &ViewContext<Self>) -> ContentDialog {
        let step = self.assist_step;
        let body: View = match step {
            0 => TextBox::new().header(TextBlock::new().text("Machine name")).text(&self.name).on_text_changed(cx.callback(Msg::Name)).into(),
            1 => RadioButtons::new()
                .header(TextBlock::new().text("Machine type"))
                .items_source(KINDS)
                .selected_index(self.assist_kind)
                .on_selection_changed(cx.callback(Msg::AssistKind))
                .into(),
            _ => format!("Create {} as {}?", self.name, KINDS[self.assist_kind.unwrap_or(0)]).into(),
        };
        let mut d = ContentDialog::new()
            .title(format!("New machine ({} of 3)", step + 1))
            .primary_button_text(if step == 2 { "Create" } else { "Next" })
            .close_button_text("Cancel")
            .is_open(self.assist_open)
            .on_closed(cx.callback(Msg::AssistClosed))
            .content(body);
        if step > 0 {
            d = d.secondary_button_text("Back");
        }
        d
    }

    pub fn assist_closed(&mut self, r: ContentDialogResult, cx: &ComponentContext<Self>) {
        println!("shell: assistant step {} closed {r:?}", self.assist_step);
        self.assist_open = false;
        let again = match r {
            ContentDialogResult::Primary if self.assist_step < 2 => {
                self.assist_step += 1;
                true
            }
            ContentDialogResult::Primary => {
                self.status = format!("created {}", self.name);
                self.tabs.push(crate::Tab { id: self.next_tab, title: self.name.clone() });
                self.next_tab += 1;
                false
            }
            ContentDialogResult::Secondary => {
                self.assist_step = self.assist_step.saturating_sub(1);
                true
            }
            _ => false,
        };
        if again {
            println!("shell: scheduling reopen");
            let _ = cx.set_local_timeout(std::time::Duration::from_millis(150), || Msg::AssistReopen);
        }
    }
}
