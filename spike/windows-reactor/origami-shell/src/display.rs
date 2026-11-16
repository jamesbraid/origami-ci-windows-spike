//! Machine tabs and the display: SwapChainPanel, pointer capture, keyboard forwarding.
use crate::core::{self, Surface};
use crate::{Msg, Shell, Tab};
use std::cell::RefCell;
use std::rc::Rc;
use windows_reactor::*;

impl Shell {
    pub fn tabs_page(&self, cx: &ViewContext<Self>) -> View {
        TabView::new()
            .automation_id("machine-tabs")
            .automation_name("Machines")
            .selected_index(self.selected)
            .can_reorder_tabs(true)
            .is_add_tab_button_visible(true)
            .on_add_tab_button_click(cx.message(Msg::TabAdd))
            .on_selection_changed(cx.callback(Msg::TabSelected))
            .on_close_requested(cx.callback(Msg::TabClose))
            .on_reordered(cx.callback(Msg::TabReorder))
            .keyed_tab_items(self.tabs.iter().map(|t| {
                keyed(
                    t.id,
                    TabViewItem::new().header(&t.title).is_closable(true).content(if t.id == 1 {
                        self.display(cx)
                    } else {
                        self.stopped(t)
                    }),
                )
            }))
            .into()
    }

    fn stopped(&self, t: &Tab) -> View {
        StackPanel::new()
            .spacing(8.0)
            .margin(24.0)
            .children((
                TextBlock::new().text(format!("{} is stopped", t.title)).font_size(20.0),
                "No display until the machine starts.",
            ))
            .into()
    }

    /// The display. The Border is the focus and input target because SwapChainPanel is not a
    /// Control; pointer capture on press keeps drags flowing after the pointer leaves it.
    fn display(&self, cx: &ViewContext<Self>) -> View {
        let core = self.core.clone();
        // At this windows-rs commit pointer callbacks are payload callbacks: they run on the UI
        // thread as the event arrives and can only queue a message, not mark the event handled.
        let c1 = core.clone();
        let pressed = cx.callback(move |i: PointerEventInfo| {
            let mut c = c1.borrow_mut();
            c.pointer("press", &i);
            c.grabbed = true;
            Msg::Touch
        });
        let (c2, n) = (core.clone(), std::cell::Cell::new(0u32));
        let moved = cx.callback(move |i: PointerEventInfo| {
            c2.borrow_mut().pointer("move", &i);
            n.set(n.get() + 1);
            if n.get() % 10 == 0 { Msg::Touch } else { Msg::Quiet }
        });
        let c3 = core.clone();
        let released = cx.callback(move |i: PointerEventInfo| {
            c3.borrow_mut().pointer("release", &i);
            Msg::Touch
        });
        let (c4, t4) = (core.clone(), cx.sender());
        let lost = move || {
            c4.borrow_mut().say("pointer capture lost".into());
            t4.send(Msg::Touch);
        };
        let c5 = core.clone();
        let chars = RoutedCallback::new(move |i: CharacterEventInfo| {
            c5.borrow_mut().character(i.character);
            true
        });
        Border::new()
            .automation_id("display")
            .automation_name("Machine display")
            .capture_pointer_on_press(true)
            .is_tab_stop(true)
            .on_pointer_pressed(pressed)
            .on_pointer_moved(moved)
            .on_pointer_released(released)
            .on_pointer_capture_lost(lost)
            .on_character_received(chars)
            .content(SwapChainPanel::new().automation_id("display-panel").automation_name("Octane2 display").element_ref(&self.panel))
            .into()
    }

    /// Bind the core's swap chain to the panel once it has a size, then present every frame.
    pub fn bind_surface(&self, cx: &mut ViewContext<Self>) {
        let panel = self.panel.clone();
        let core = self.core.clone();
        let tx = cx.sender();
        cx.use_effect("surface", (), move || {
            let slot: Rc<RefCell<Option<Rc<RefCell<Surface>>>>> = Rc::new(RefCell::new(None));
            let started = Rc::new(RefCell::new(false));
            let panel2 = panel.clone();
            let obs = panel.observe_surface(move |event| match event {
                SwapChainPanelEvent::Metrics { width, height, scale_x, .. } => {
                    {
                        let mut c = core.borrow_mut();
                        if c.panel != (width, height) {
                            println!("shell: panel {width}x{height} dip, scale {scale_x}");
                        }
                        c.panel = (width, height);
                    }
                    if std::mem::replace(&mut *started.borrow_mut(), true) {
                        return;
                    }
                    let (slot, tx) = (slot.clone(), tx.clone());
                    core::attach(&panel2, 1, move |r| match r {
                        Ok(s) => {
                            *slot.borrow_mut() = Some(s);
                            tx.send(Msg::Surface(Ok(())));
                        }
                        Err(e) => {
                            tx.send(Msg::Surface(Err(e)));
                        }
                    });
                }
                SwapChainPanelEvent::Rendering => {
                    let s = slot_clone(&slot);
                    if let Some(s) = s {
                        let mut c = core.borrow_mut();
                        c.frame += 1;
                        s.borrow_mut().frame(&c, false);
                    }
                }
            });
            Some(Box::new(move || drop(obs)))
        });
    }

    pub fn recent_page(&self) -> View {
        StackPanel::new()
            .margin(24.0)
            .spacing(8.0)
            .children((
                TextBlock::new().text("Recent disks").font_size(20.0),
                format!("Last picked: {}", self.last_file),
            ))
            .into()
    }

    pub fn add_tab(&mut self) {
        let id = self.next_tab;
        self.next_tab += 1;
        self.tabs.push(Tab { id, title: format!("Machine {id}") });
        self.selected = Some(self.tabs.len() - 1);
    }

    pub fn close_tab(&mut self, key: &Key) {
        let Some(i) = self.tabs.iter().position(|t| Key::from(t.id) == *key) else { return };
        self.tabs.remove(i);
        self.selected = if self.tabs.is_empty() { None } else { Some(i.min(self.tabs.len() - 1)) };
    }

    pub fn reorder_tabs(&mut self, order: &[Key]) {
        let sel = self.selected.map(|i| self.tabs[i].id);
        self.tabs.sort_by_key(|t| order.iter().position(|k| *k == Key::from(t.id)));
        self.selected = sel.and_then(|id| self.tabs.iter().position(|t| t.id == id));
    }
}

fn slot_clone(slot: &Rc<RefCell<Option<Rc<RefCell<Surface>>>>>) -> Option<Rc<RefCell<Surface>>> {
    slot.borrow().clone()
}
