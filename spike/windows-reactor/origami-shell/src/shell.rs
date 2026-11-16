//! The frame: menu bar, toolbar, navigation, status bar, keyboard accelerators.
use crate::{Msg, Shell};
use windows_reactor::*;

const CTRL: u32 = 0;

impl Shell {
    pub fn root(&self, cx: &ViewContext<Self>) -> View {
        let rows = [GridLength::Auto, GridLength::Auto, GridLength::STAR, GridLength::Auto];
        let grid = Grid::new()
            .rows(rows)
            // Reactor's own accelerator type: Ctrl+N and Ctrl+R are two of its 3 letters.
            .key_accelerators(KeyAccelerators::new([
                KeyAccelerator::new(AcceleratorKey::N, AcceleratorModifiers::Control, cx.message(Msg::Shortcut("Ctrl+N (KeyAccelerator)"))),
                KeyAccelerator::new(AcceleratorKey::R, AcceleratorModifiers::Control, cx.message(Msg::Shortcut("Ctrl+R (KeyAccelerator)"))),
            ]))
            .children((self.menu_bar(cx), self.command_bar(cx), self.navigation(cx), self.status_bar()));

        let core = self.core.clone();
        let tx = cx.sender();
        // Window-wide shortcuts for every key reactor's KeyAccelerator cannot name. While the
        // display holds the keyboard grab, every key goes to the core and nothing else sees it.
        let keys = RoutedCallback::new(move |info: KeyEventInfo| {
            let ctrl = info.modifiers.contains(InputModifiers::CONTROL);
            let alt = info.modifiers.contains(InputModifiers::ALT);
            let shift = info.modifiers.contains(InputModifiers::SHIFT);
            let k = info.key.0;
            if core.borrow().grabbed {
                if ctrl && alt && k == 0x47 {
                    tx.send(Msg::Grab(false));
                } else {
                    core.borrow_mut().key(&info);
                    tx.send(Msg::Touch);
                }
                return true;
            }
            let hit = match (k, ctrl, shift) {
                (0x4F, true, false) => Some(Msg::PickFile),
                (0x57, true, false) => Some(Msg::Shortcut("Ctrl+W close tab")),
                (0xBC, true, false) => Some(Msg::Nav(Some("settings".into()))),
                (0x4E, true, true) => Some(Msg::AssistOpen),
                (0x7A, false, false) => Some(Msg::Shortcut("F11 full screen")),
                _ => None,
            };
            match hit {
                Some(m) => {
                    tx.send(m);
                    true
                }
                None => false,
            }
        });
        let _ = CTRL;
        Border::new()
            .on_preview_key_down(keys)
            .content(grid)
            .content_dialog(self.confirm_dialog(cx))
            .into()
    }

    pub fn menu_bar(&self, cx: &ViewContext<Self>) -> MenuBar {
        let cb = || cx.callback(Msg::Menu);
        let file = Menu::new(
            [
                MenuItem::item("file.new", "New machine...  (Ctrl+Shift+N)"),
                MenuItem::item("file.open", "Open disk image...  (Ctrl+O)"),
                MenuItem::separator("file.sep"),
                MenuItem::item("file.popout", "Pop out tab"),
                MenuItem::item("file.close", "Close tab  (Ctrl+W)"),
                MenuItem::item("file.settings", "Settings  (Ctrl+,)"),
            ],
            cb(),
        );
        let view = Menu::new(
            [
                MenuItem::submenu(
                    "view.theme",
                    "Theme",
                    [
                        MenuItem::item("view.theme.system", "Use system setting"),
                        MenuItem::item("view.theme.light", "Light"),
                        MenuItem::item("view.theme.dark", "Dark"),
                    ],
                ),
                MenuItem::item("view.console", "Console window"),
            ],
            cb(),
        );
        let machine = Menu::new(
            [
                MenuItem::item("machine.start", "Start"),
                MenuItem::item("machine.reset", "Reset..."),
                MenuItem::item("machine.release", "Release input  (Ctrl+Alt+G)"),
            ],
            cb(),
        );
        MenuBar::new()
            .automation_id("menu-bar")
            .automation_name("Main menu")
            .grid_row(0)
            .items((
                MenuBarItem::new().title("File").menu(file),
                MenuBarItem::new().title("View").menu(view),
                MenuBarItem::new().title("Machine").menu(machine),
            ))
    }

    pub fn menu(&mut self, key: &str, cx: &ComponentContext<Self>) {
        println!("shell: menu {key}");
        let tx = cx.sender();
        let msg = match key {
            "file.new" => Msg::AssistOpen,
            "file.open" => Msg::PickFile,
            "file.popout" => Msg::PopOut,
            "file.close" => Msg::Shortcut("close tab (menu)"),
            "file.settings" => Msg::Nav(Some("settings".into())),
            "view.theme.system" => Msg::SetTheme(WindowTheme::System),
            "view.theme.light" => Msg::SetTheme(WindowTheme::Light),
            "view.theme.dark" => Msg::SetTheme(WindowTheme::Dark),
            "view.console" => Msg::Console,
            "machine.reset" => Msg::ConfirmOpen(true),
            "machine.release" => Msg::Grab(false),
            "machine.start" => Msg::Shortcut("start (menu)"),
            _ => return,
        };
        tx.send(msg);
    }

    pub fn command_bar(&self, cx: &ViewContext<Self>) -> CommandBar {
        CommandBar::new()
            .automation_id("toolbar")
            .automation_name("Machine toolbar")
            .grid_row(1)
            .owned_commands(
                [
                    CommandBarCommand::button_with_icon("tb.start", "Start", Symbol::Play),
                    CommandBarCommand::button_with_icon("tb.pause", "Pause", Symbol::Pause),
                    CommandBarCommand::button_with_icon("tb.stop", "Stop", Symbol::Stop),
                    CommandBarCommand::separator("tb.sep"),
                    CommandBarCommand::button_with_icon("tb.popout", "Pop out", Symbol::BackToWindow),
                    CommandBarCommand::button_with_icon("tb.open", "Open image", Symbol::OpenFile),
                ],
                [CommandBarCommand::button("tb.console", "Console window")],
                cx.callback(|key: Key| match key.as_str().unwrap_or("") {
                    "tb.popout" => Msg::PopOut,
                    "tb.open" => Msg::PickFile,
                    "tb.console" => Msg::Console,
                    "tb.start" => Msg::Shortcut("toolbar start"),
                    "tb.pause" => Msg::Shortcut("toolbar pause"),
                    _ => Msg::Shortcut("toolbar stop"),
                }),
            )
    }

    pub fn navigation(&self, cx: &ViewContext<Self>) -> NavigationView {
        let is = |t: &str| &*self.nav == t;
        NavigationView::new()
            .automation_id("sidebar")
            .automation_name("Library sidebar")
            .grid_row(2)
            .pane_display_mode(NavigationViewPaneDisplayMode::Left)
            .open_pane_length(220.0)
            .is_settings_visible(false)
            .is_back_button_visible(NavigationViewBackButtonVisible::Collapsed)
            .on_selected_tag_changed(cx.callback(Msg::Nav))
            .menu_items((
                NavigationViewItem::new().tag("library").content("Library").icon(Symbol::Library).is_selected(is("library")),
                NavigationViewItem::new().tag("recent").content("Recent disks").icon(Symbol::Document).is_selected(is("recent")),
            ))
            .footer_menu_items((
                NavigationViewItem::new().tag("settings").content("Settings").icon(Symbol::Setting).is_selected(is("settings")),
            ))
            .content(match &*self.nav {
                "settings" => self.settings_page(cx),
                "recent" => self.recent_page(),
                _ => self.tabs_page(cx),
            })
    }

    pub fn status_bar(&self) -> View {
        let c = self.core.borrow();
        Border::new()
            .grid_row(3)
            .padding(Thickness::xy(12.0, 4.0))
            .background(ThemeBrush::CardBackground)
            .content(
                TextBlock::new()
                    .automation_id("status-bar")
                    .text(format!(
                        "{}  |  {}  |  grab={} keys={} frame={}  |  theme {:?}/{:?}",
                        self.status, self.surface_state, c.grabbed, c.keys, c.frame, self.theme, self.scheme
                    )),
            )
            .into()
    }
}
