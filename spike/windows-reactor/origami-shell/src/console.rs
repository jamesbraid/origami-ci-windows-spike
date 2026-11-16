//! Second window: a pop-out machine or the Console, each with its own SwapChainPanel and
//! swap chain. The console draws its own cells and takes typed characters with no text control.
use crate::core::{self, Core, Surface};
use std::cell::RefCell;
use std::rc::Rc;
use windows_reactor::*;

#[derive(Clone, PartialEq)]
pub struct PopInput {
    pub title: String,
    pub console: bool,
}

#[derive(Clone)]
pub enum PopMsg {
    Bound(Result<(), String>),
    Typed,
}

pub struct Pop {
    panel: ElementRef<SwapChainPanel>,
    core: Rc<RefCell<Core>>,
    state: String,
}

impl Component for Pop {
    type Input = PopInput;
    type Message = PopMsg;

    fn create(_: &PopInput, _: &ComponentContext<Self>) -> Self {
        Self { panel: ElementRef::new(), core: Rc::new(RefCell::new(Core::default())), state: "binding".into() }
    }

    fn update(&mut self, m: PopMsg, _: &ComponentContext<Self>) {
        match m {
            PopMsg::Bound(r) => self.state = format!("{r:?}"),
            PopMsg::Typed => {}
        }
    }

    fn view(&self, input: &PopInput, cx: &mut ViewContext<Self>) -> View {
        cx.window_title(&input.title);
        let (panel, core, tx, console) = (self.panel.clone(), self.core.clone(), cx.sender(), input.console);
        cx.use_effect("surface", (), move || {
            let slot: Rc<RefCell<Option<Rc<RefCell<Surface>>>>> = Rc::new(RefCell::new(None));
            let started = Rc::new(RefCell::new(false));
            let p2 = panel.clone();
            let obs = panel.observe_surface(move |e| match e {
                SwapChainPanelEvent::Metrics { .. } => {
                    if std::mem::replace(&mut *started.borrow_mut(), true) {
                        return;
                    }
                    let (slot, tx) = (slot.clone(), tx.clone());
                    core::attach(&p2, 2, move |r| match r {
                        Ok(s) => {
                            *slot.borrow_mut() = Some(s);
                            tx.send(PopMsg::Bound(Ok(())));
                        }
                        Err(e) => {
                            tx.send(PopMsg::Bound(Err(e)));
                        }
                    });
                }
                SwapChainPanelEvent::Rendering => {
                    if let Some(s) = slot.borrow().clone() {
                        let mut c = core.borrow_mut();
                        c.frame += 1;
                        s.borrow_mut().frame(&c, console);
                    }
                }
                _ => {}
            });
            Some(Box::new(move || drop(obs)))
        });
        let (c, tx) = (self.core.clone(), cx.sender());
        let chars = RoutedCallback::new(move |i: CharacterEventInfo| {
            c.borrow_mut().character(i.character);
            tx.send(PopMsg::Typed);
            true
        });
        Border::new()
            .is_tab_stop(true)
            .on_character_received(chars)
            .content(SwapChainPanel::new().automation_id("pop-panel").automation_name(&input.title).element_ref(&self.panel))
            .into()
    }
}
