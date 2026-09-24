use gpui::*;

actions!(app, [About]);

#[cfg(target_os = "macos")]
pub fn show() {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{
        NSAboutPanelOptionApplicationName, NSAboutPanelOptionApplicationVersion,
        NSAboutPanelOptionVersion, NSApplication,
    };
    use objc2_foundation::{NSDictionary, NSString};

    let app = NSApplication::sharedApplication(
        MainThreadMarker::new().expect("About runs on the main thread"),
    );
    let name = NSString::from_str("AgentInc");
    let version = NSString::from_str(&ainc_release::identity::version());
    let commit = ainc_release::identity::COMMIT;
    let build = if ainc_release::identity::PRODUCTION {
        ainc_release::BUILD
    } else {
        &commit[..commit.len().min(12)]
    };
    let build = NSString::from_str(build);
    let keys = unsafe {
        [
            NSAboutPanelOptionApplicationName,
            NSAboutPanelOptionApplicationVersion,
            NSAboutPanelOptionVersion,
        ]
    };
    let options = NSDictionary::from_slices(&keys, &[&*name, &*version, &*build]);
    unsafe {
        let _: () = objc2::msg_send![&*app, orderFrontStandardAboutPanelWithOptions: &*options];
    }
}

#[cfg(not(target_os = "macos"))]
pub fn show() {}

pub struct CommitTooltip;
impl Render for CommitTooltip {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .pb(px(80.))
            .child(crate::ui::tooltip_shell(ainc_release::identity::COMMIT))
    }
}
