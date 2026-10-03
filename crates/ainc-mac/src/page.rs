//! What every page gives the shell: a view, the dialog it is showing, the
//! focus ring of that dialog, its drafts across an update, and the hooks the
//! shell calls as routes change. The shell holds pages as [`PageHandle`]s and
//! iterates; it never names one.
use crate::{
    input::TextInput,
    overlay::Overlay,
    routes::{Destination, Route},
    ui::OverlayHost,
};
use gpui::{prelude::*, *};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{cell::RefCell, rc::Rc};

pub trait Page: Render + EventEmitter<Destination> + Sized + 'static {
    const ROUTE: Route;
    /// The page's name, from the one catalogue the sidebar and palette use.
    fn title(&self) -> SharedString {
        Self::ROUTE.label().into()
    }
    /// The dialog to draw on the shell's scrim while this page's dialog is active.
    fn overlay(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> Option<AnyElement> {
        None
    }
    /// Tab order inside the open dialog.
    fn focus_handles(&self, _cx: &App) -> Vec<FocusHandle> {
        Vec::new()
    }
    /// What the person was typing, kept across an update install. Err refuses
    /// the install while a change is in flight.
    fn drafts(&self, _cx: &App) -> anyhow::Result<Drafts> {
        Ok(Drafts::default())
    }
    fn restore(&mut self, _drafts: Drafts, _cx: &mut Context<Self>) {}
    /// A destination was chosen somewhere in the app; select the record if it is yours.
    fn open(&mut self, _to: &Destination, _window: &mut Window, _cx: &mut Context<Self>) {}
    /// The page became (or stopped being) the current route.
    fn shown(&mut self, _shown: bool, _cx: &mut Context<Self>) {}
}

/// Text and selections a page keeps across an update install, by stable key.
#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Drafts(serde_json::Map<String, serde_json::Value>);
impl Drafts {
    pub fn set(&mut self, key: &str, value: impl Serialize) {
        if let Ok(value) = serde_json::to_value(value) {
            self.0.insert(key.to_owned(), value);
        }
    }
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.0
            .get(key)
            .cloned()
            .and_then(|value| serde_json::from_value(value).ok())
    }
    pub fn text(&mut self, key: &str, input: &Entity<TextInput>, cx: &App) {
        self.set(key, input.read(cx).content.to_string());
    }
    pub fn restore_text(&self, key: &str, input: &Entity<TextInput>, cx: &mut App) {
        if let Some(text) = self.get::<String>(key) {
            input.update(cx, |input, cx| input.set_text(&text, cx));
        }
    }
}

/// A page's own dialogs and popovers, registered with the window's one
/// [`OverlayHost`] under the page's route. `active()` is `None` once the shell
/// dismissed the surface, however the page's record of it reads. Popovers
/// (selects and menus) are keyed by the control's id, so one is open at a time
/// and the shell closes it on Escape or a click outside.
pub struct PageOverlays<D: Copy> {
    host: Rc<RefCell<OverlayHost<Overlay>>>,
    route: Route,
    open: Option<D>,
    popover: Option<SharedString>,
}
impl<D: Copy> PageOverlays<D> {
    pub fn new(host: Rc<RefCell<OverlayHost<Overlay>>>, route: Route) -> Self {
        Self {
            host,
            route,
            open: None,
            popover: None,
        }
    }
    pub fn active(&self) -> Option<D> {
        let shown = matches!(
            self.host.borrow().active(),
            Some(Overlay::Dialog(route)) if route == self.route
        );
        self.open.filter(|_| shown)
    }
    /// Whether the select or menu with this id is the open popover.
    pub fn popover_open(&self, id: &str) -> bool {
        self.host.borrow().popover() == Some(Overlay::Popover(self.route))
            && self.popover.as_deref() == Some(id)
    }
    /// Open the popover with this id, or close it when it is the open one.
    /// Returns whether it is open afterwards.
    pub fn toggle_popover(&mut self, id: impl Into<SharedString>) -> bool {
        let id = id.into();
        if self.popover_open(&id) {
            self.close_popover();
            return false;
        }
        let mut host = self.host.borrow_mut();
        // A shell popover (the user menu) gives way to a page's.
        if host.active().is_some_and(Overlay::is_popover) {
            host.close();
        }
        host.open_popover(Overlay::Popover(self.route));
        self.popover = Some(id);
        true
    }
    pub fn close_popover(&mut self) {
        if self.popover.take().is_some() {
            self.host.borrow_mut().close_popover();
        }
    }
    /// A modal dialog on the shell's scrim, with its first focus target.
    pub fn open_dialog(
        &mut self,
        dialog: D,
        focus: FocusHandle,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.open = Some(dialog);
        self.host
            .borrow_mut()
            .open(Overlay::Dialog(self.route), window, cx, Some(focus));
    }
    pub fn close(&mut self) {
        self.open = None;
        self.popover = None;
        self.host.borrow_mut().close();
    }
    /// Close the dialog and any popover over it, returning focus.
    pub fn dismiss(&mut self, window: &mut Window, cx: &mut App) {
        self.open = None;
        self.popover = None;
        let mut host = self.host.borrow_mut();
        host.close_popover();
        host.dismiss(window, cx);
    }
}

/// A page as the shell sees it: its route, view and the [`Page`] hooks, with
/// the concrete type erased.
pub struct PageHandle {
    route: Route,
    view: AnyView,
    ops: Rc<dyn PageOps>,
    _subscriptions: [Subscription; 2],
}
impl PageHandle {
    /// Wrap a page; `on_destination` receives what it emits.
    pub fn new<P: Page, S: 'static>(
        entity: Entity<P>,
        cx: &mut Context<S>,
        on_destination: impl Fn(&mut S, Destination, &mut Context<S>) + 'static,
    ) -> Self {
        let subscriptions = [
            cx.observe(&entity, |_, _, cx| cx.notify()),
            cx.subscribe(&entity, move |host, _, to: &Destination, cx| {
                on_destination(host, to.clone(), cx)
            }),
        ];
        Self {
            route: P::ROUTE,
            view: entity.clone().into(),
            ops: Rc::new(entity),
            _subscriptions: subscriptions,
        }
    }
    pub fn route(&self) -> Route {
        self.route
    }
    pub fn view(&self) -> AnyView {
        self.view.clone()
    }
    /// The concrete page, for the few callers that drive one directly.
    pub fn entity<P: Page>(&self) -> Entity<P> {
        self.view
            .clone()
            .downcast::<P>()
            .unwrap_or_else(|_| panic!("{:?} is not that page", self.route))
    }
    pub fn overlay(&self, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
        self.ops.overlay(window, cx)
    }
    pub fn focus_handles(&self, cx: &App) -> Vec<FocusHandle> {
        self.ops.focus_handles(cx)
    }
    pub fn drafts(&self, cx: &App) -> anyhow::Result<Drafts> {
        self.ops.drafts(cx)
    }
    pub fn restore(&self, drafts: Drafts, cx: &mut App) {
        self.ops.restore(drafts, cx)
    }
    pub fn open(&self, to: &Destination, window: &mut Window, cx: &mut App) {
        self.ops.open(to, window, cx)
    }
    pub fn shown(&self, shown: bool, cx: &mut App) {
        self.ops.shown(shown, cx)
    }
    /// Redraw, after a change the page cannot observe (the type scale).
    pub fn refresh(&self, cx: &mut App) {
        self.ops.refresh(cx)
    }
}

trait PageOps {
    fn overlay(&self, window: &mut Window, cx: &mut App) -> Option<AnyElement>;
    fn focus_handles(&self, cx: &App) -> Vec<FocusHandle>;
    fn drafts(&self, cx: &App) -> anyhow::Result<Drafts>;
    fn restore(&self, drafts: Drafts, cx: &mut App);
    fn open(&self, to: &Destination, window: &mut Window, cx: &mut App);
    fn shown(&self, shown: bool, cx: &mut App);
    fn refresh(&self, cx: &mut App);
}
impl<P: Page> PageOps for Entity<P> {
    fn overlay(&self, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
        self.update(cx, |page, cx| page.overlay(window, cx))
    }
    fn focus_handles(&self, cx: &App) -> Vec<FocusHandle> {
        self.read(cx).focus_handles(cx)
    }
    fn drafts(&self, cx: &App) -> anyhow::Result<Drafts> {
        self.read(cx).drafts(cx)
    }
    fn restore(&self, drafts: Drafts, cx: &mut App) {
        self.update(cx, |page, cx| page.restore(drafts, cx))
    }
    fn open(&self, to: &Destination, window: &mut Window, cx: &mut App) {
        self.update(cx, |page, cx| page.open(to, window, cx))
    }
    fn shown(&self, shown: bool, cx: &mut App) {
        self.update(cx, |page, cx| page.shown(shown, cx))
    }
    fn refresh(&self, cx: &mut App) {
        self.update(cx, |_, cx| cx.notify())
    }
}

#[cfg(test)]
mod tests {
    use super::Drafts;
    #[test]
    fn drafts_round_trip_through_json() {
        let mut drafts = Drafts::default();
        drafts.set("selected", Some(4_i64));
        drafts.set("list", true);
        let text = serde_json::to_string(&drafts).unwrap();
        let restored: Drafts = serde_json::from_str(&text).unwrap();
        assert_eq!(restored.get::<Option<i64>>("selected"), Some(Some(4)));
        assert_eq!(restored.get::<bool>("list"), Some(true));
        assert_eq!(restored.get::<String>("missing"), None);
    }
}
