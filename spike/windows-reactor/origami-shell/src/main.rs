//! Origami Windows shell spike on Microsoft's windows-reactor (WinUI 3 from Rust).
//! Every must-have from the UX design appears at least once; see the README for the verdicts.
mod console;
mod core;
mod d3d;
mod display;
mod settings;
mod shell;

use crate::core::{Core, SharedCore, Surface};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use windows_reactor::*;

#[derive(Clone)]
pub enum Msg {
    Nav(Option<Rc<str>>),
    TabSelected(Option<usize>),
    TabClose(Key),
    TabReorder(Vec<Key>),
    TabAdd,
    Menu(Key),
    /// A shortcut the shell handled, with its name, for the status bar and the log.
    Shortcut(&'static str),
    PickFile,
    FileChosen(Option<PathBuf>),
    FileFailed(String),
    SetTheme(WindowTheme),
    Scheme(ColorScheme),
    Name(Rc<str>),
    Grab(bool),
    Audio(Option<usize>),
    Scaling(Option<usize>),
    Card(usize),
    ConfirmOpen(bool),
    ConfirmClosed(ContentDialogResult),
    AssistOpen,
    AssistClosed(ContentDialogResult),
    AssistReopen,
    AssistKind(Option<usize>),
    Surface(Result<(), String>),
    Touch,
    PopOut,
    Console,
}

pub struct Tab {
    pub id: u32,
    pub title: String,
}

pub struct Shell {
    pub nav: Rc<str>,
    pub tabs: Vec<Tab>,
    pub selected: Option<usize>,
    pub next_tab: u32,
    pub theme: WindowTheme,
    pub scheme: Option<ColorScheme>,
    pub panel: ElementRef<SwapChainPanel>,
    pub core: SharedCore,
    pub status: String,
    pub surface_state: String,
    // settings form
    pub name: String,
    pub audio: Option<usize>,
    pub scaling: Option<usize>,
    pub card: usize,
    // dialogs
    pub confirm_open: bool,
    pub assist_open: bool,
    pub assist_step: usize,
    pub assist_kind: Option<usize>,
    pub last_file: String,
}

impl Component for Shell {
    type Input = ();
    type Message = Msg;

    fn create(_: &(), _: &ComponentContext<Self>) -> Self {
        Self {
            nav: "library".into(),
            tabs: vec![
                Tab { id: 1, title: "Octane2".into() },
                Tab { id: 2, title: "O2".into() },
            ],
            selected: Some(0),
            next_tab: 3,
            theme: WindowTheme::System,
            scheme: None,
            panel: ElementRef::new(),
            core: Rc::new(RefCell::new(Core::default())),
            status: "Ready".into(),
            surface_state: "no swap chain yet".into(),
            name: "Octane2 (lab)".into(),
            audio: Some(0),
            scaling: Some(0),
            card: 0,
            confirm_open: false,
            assist_open: false,
            assist_step: 0,
            assist_kind: Some(0),
            last_file: "(none)".into(),
        }
    }

    fn update(&mut self, msg: Msg, cx: &ComponentContext<Self>) {
        match msg {
            Msg::Nav(tag) => {
                if let Some(t) = tag {
                    self.nav = t;
                }
            }
            Msg::TabSelected(i) => self.selected = i,
            Msg::TabClose(key) => self.close_tab(&key),
            Msg::TabReorder(order) => self.reorder_tabs(&order),
            Msg::TabAdd => self.add_tab(),
            Msg::Menu(key) => self.menu(key.as_str().unwrap_or(""), cx),
            Msg::Shortcut(name) => {
                self.status = format!("shortcut: {name}");
                println!("shell: shortcut {name}");
            }
            Msg::PickFile => {
                println!("shell: opening file picker");
                let staged = windows_pickers::OpenFilePicker::new()
                    .title("Open disk image")
                    .filter_extensions("Disk images", ["img", "iso", "chd"])
                    .filter_all()
                    .request(cx, |r| match r {
                        Ok(p) => Msg::FileChosen(p),
                        Err(e) => Msg::FileFailed(e.to_string()),
                    });
                if !staged {
                    self.status = "file picker was not staged".into();
                }
            }
            Msg::FileChosen(p) => {
                self.last_file = p.map(|p| p.display().to_string()).unwrap_or("(cancelled)".into());
                println!("shell: picker returned {}", self.last_file);
                self.status = format!("disk image: {}", self.last_file);
            }
            Msg::FileFailed(e) => {
                println!("shell: picker failed {e}");
                self.status = format!("picker failed: {e}");
            }
            Msg::SetTheme(t) => {
                self.theme = t;
                println!("shell: theme {t:?}");
            }
            Msg::Scheme(s) => {
                println!("shell: color scheme {s:?}");
                self.scheme = Some(s);
            }
            Msg::Name(n) => self.name = n.to_string(),
            Msg::Grab(g) => {
                self.core.borrow_mut().grabbed = g;
                self.status = if g { "input grabbed (Ctrl+Alt+G releases)".into() } else { "input released".into() };
                println!("shell: grabbed={g}");
            }
            Msg::Audio(i) => self.audio = i,
            Msg::Scaling(i) => self.scaling = i,
            Msg::Card(i) => self.card = i,
            Msg::ConfirmOpen(o) => self.confirm_open = o,
            Msg::ConfirmClosed(r) => {
                self.confirm_open = false;
                self.status = format!("reset confirm: {r:?}");
                println!("shell: confirm dialog {r:?}");
            }
            Msg::AssistOpen => {
                self.assist_step = 0;
                self.assist_open = true;
            }
            Msg::AssistClosed(r) => self.assist_closed(r, cx),
            Msg::AssistReopen => self.assist_open = true,
            Msg::AssistKind(k) => self.assist_kind = k,
            Msg::Surface(r) => {
                self.surface_state = match r {
                    Ok(()) => "swap chain bound".into(),
                    Err(e) => format!("swap chain FAILED: {e}"),
                };
                println!("shell: {}", self.surface_state);
            }
            Msg::Touch => {
                let c = self.core.borrow();
                self.status = format!("core: {}", c.last);
            }
            Msg::PopOut => {
                let title = self.selected.and_then(|i| self.tabs.get(i)).map(|t| t.title.clone()).unwrap_or("Machine".into());
                let ok = cx.open_window_with_policy::<console::Pop>(
                    console::PopInput { title, console: false },
                    WindowPolicy::new().client_size(720.0, 520.0),
                );
                println!("shell: pop-out window opened={ok}");
            }
            Msg::Console => {
                let ok = cx.open_window_with_policy::<console::Pop>(
                    console::PopInput { title: "Console".into(), console: true },
                    WindowPolicy::new().client_size(720.0, 520.0),
                );
                println!("shell: console window opened={ok}");
            }
        }
    }

    fn view(&self, _: &(), cx: &mut ViewContext<Self>) -> View {
        cx.window_title("Origami");
        cx.window_visuals(WindowVisuals::new().client_size(960.0, 640.0).theme(self.theme));
        let scheme_cb = cx.callback(Msg::Scheme);
        cx.on_color_scheme(scheme_cb);
        self.bind_surface(cx);
        self.root(cx)
    }
}

fn main() {
    println!("shell: start pid={}", std::process::id());
    let r = App::run_component::<Shell>(());
    println!("shell: app_run_returned {r:?}");
}

#[allow(dead_code)]
type _Unused = Surface;
