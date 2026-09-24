//! GPUI adapter. Transport threads never access application entities or dispatch input.
use crate::{
    protocol::*,
    transport::{self, Session},
};
use anyhow::Result;
use gpui::{
    AnyWindowHandle, App, AppContext, AsyncApp, KeyUpEvent, Keystroke, Modifiers, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, PlatformInput, Window,
};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

struct Pending {
    request: Request,
    deadline: Instant,
    response: mpsc::SyncSender<Response>,
}
/// Keep this guard alive for the opted-in app's lifetime.
pub struct Host {
    session: Arc<Session>,
    stopping: Arc<AtomicBool>,
    listener: Option<std::thread::JoinHandle<()>>,
    _task: gpui::Task<()>,
    _quit: gpui::Subscription,
}
impl gpui::Global for Host {}
impl Drop for Host {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Release);
        let _ = std::os::unix::net::UnixStream::connect(&self.session.manifest.socket);
        if let Some(listener) = self.listener.take() {
            let _ = listener.join();
        }
        self.session.close();
    }
}
impl Host {
    pub fn start(
        directory: &Path,
        title: String,
        window: AnyWindowHandle,
        cx: &mut App,
    ) -> Result<Self> {
        let (session, listener) = Session::create(directory, title)?;
        let session = Arc::new(session);
        window.update(cx, |_, window, _| window.pilot_enable())?;
        let (sender, receiver) = async_channel::bounded::<Pending>(32);
        let stopping = Arc::new(AtomicBool::new(false));
        let active = Arc::new(AtomicUsize::new(0));
        let listener_thread = {
            let session = session.clone();
            let stopping = stopping.clone();
            std::thread::spawn(move || {
                for connection in listener.incoming() {
                    if stopping.load(Ordering::Acquire) {
                        break;
                    }
                    let Ok(mut stream) = connection else { break };
                    if active.fetch_add(1, Ordering::AcqRel) >= 8 {
                        active.fetch_sub(1, Ordering::AcqRel);
                        continue;
                    }
                    let (session, sender, active) =
                        (session.clone(), sender.clone(), active.clone());
                    std::thread::spawn(move || {
                        let result = (|| -> Result<()> {
                            transport::peer_is_owner(&stream)?;
                            stream.set_read_timeout(Some(Duration::from_secs(2)))?;
                            stream.set_write_timeout(Some(Duration::from_secs(2)))?;
                            let request: Request =
                                transport::read_message(&mut stream, MAX_MESSAGE)?;
                            let id = request.id;
                            let response = match session.authenticate(&request) {
                                Err(error) => Response {
                                    version: VERSION,
                                    id,
                                    result: Reply::Error {
                                        error,
                                        snapshot: None,
                                    },
                                },
                                Ok(()) => {
                                    let (response, receive) = mpsc::sync_channel(1);
                                    if sender
                                        .try_send(Pending {
                                            request,
                                            deadline: Instant::now() + Duration::from_secs(12),
                                            response,
                                        })
                                        .is_err()
                                    {
                                        Response {
                                            version: VERSION,
                                            id,
                                            result: Reply::Error {
                                                error: Failure::new(
                                                    "busy",
                                                    "Request queue is full",
                                                ),
                                                snapshot: None,
                                            },
                                        }
                                    } else {
                                        receive.recv_timeout(Duration::from_secs(13))?
                                    }
                                }
                            };
                            transport::write_message(&mut stream, &response)
                        })();
                        // No request text/token is logged, including malformed messages.
                        let _ = result;
                        active.fetch_sub(1, Ordering::AcqRel);
                    });
                }
            })
        };
        let task_session = session.clone();
        let task = cx.spawn(async move |cx| {
            let captures = Arc::new(AtomicUsize::new(0));
            while let Ok(pending) = receiver.recv().await {
                let session = task_session.clone();
                let captures = captures.clone();
                cx.spawn(async move |cx| {
                    let result = execute(&pending, &session, window, captures, cx).await;
                    let result = match result {
                        Ok(output) => Reply::Ok { output },
                        Err(error) => Reply::Error {
                            error,
                            snapshot: current_snapshot(window, &session, cx).ok(),
                        },
                    };
                    let _ = pending.response.send(Response {
                        version: VERSION,
                        id: pending.request.id,
                        result,
                    });
                })
                .detach();
            }
        });
        let quit_session = session.clone();
        let quit_stopping = stopping.clone();
        let quit = cx.on_app_quit(move |_| {
            quit_stopping.store(true, Ordering::Release);
            let _ = std::os::unix::net::UnixStream::connect(&quit_session.manifest.socket);
            quit_session.close();
            async {}
        });
        Ok(Self {
            session,
            stopping,
            listener: Some(listener_thread),
            _task: task,
            _quit: quit,
        })
    }
}
fn failure(error: impl std::fmt::Display) -> Failure {
    Failure::new("host_error", error.to_string())
}
fn current_snapshot(
    handle: AnyWindowHandle,
    session: &Session,
    cx: &mut AsyncApp,
) -> Result<Snapshot, Failure> {
    cx.update_window(handle, |_, window, cx| {
        if window.pilot_needs_draw() {
            window.draw(cx).clear(cx);
        }
        snapshot(window, session)
    })
    .map_err(|_| Failure::new("window_closed", "Selected window is closed"))?
}

async fn execute(
    pending: &Pending,
    session: &Session,
    handle: AnyWindowHandle,
    captures: Arc<AtomicUsize>,
    cx: &mut AsyncApp,
) -> Result<Output, Failure> {
    if Instant::now() > pending.deadline {
        return Err(Failure::new(
            "deadline_exceeded",
            "Request expired before dispatch",
        ));
    }
    match &pending.request.command {
        Command::Hello => Ok(Output::Hello {
            session: session.manifest.session.clone(),
            window: session.manifest.window.clone(),
            title: session.manifest.title.clone(),
            capabilities: ["snapshot", "click", "press", "type", "wait", "screenshot"]
                .map(str::to_owned)
                .to_vec(),
        }),
        Command::Snapshot => Ok(Output::Snapshot {
            snapshot: current_snapshot(handle, session, cx)?,
        }),
        Command::Wait {
            condition,
            timeout_ms,
        } => {
            let deadline = Instant::now() + Duration::from_millis(*timeout_ms);
            loop {
                let snapshot = current_snapshot(handle, session, cx)?;
                if condition.matches(&snapshot) {
                    return Ok(Output::Snapshot { snapshot });
                }
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Err(Failure::new(
                        "deadline_exceeded",
                        "Condition was not observed before deadline",
                    ));
                }
                // Schedule a committed observation on frame demand; timeout remains independent
                // of WindowServer delivery (e.g. a minimized/closed window).
                let (send, receive) = futures::channel::oneshot::channel();
                cx.update_window(handle, |_, window, _| {
                    window.on_next_frame(move |window, cx| {
                        if window.pilot_needs_draw() {
                            window.draw(cx).clear(cx);
                        }
                        let _ = send.send(());
                    })
                })
                .map_err(|_| Failure::new("window_closed", "Selected window is closed"))?;
                let timer = cx.background_executor().timer(remaining);
                if matches!(
                    futures::future::select(receive, Box::pin(timer)).await,
                    futures::future::Either::Right(_)
                ) {
                    return Err(Failure::new(
                        "deadline_exceeded",
                        "Condition was not observed before deadline",
                    ));
                }
            }
        }
        Command::Screenshot => {
            let capture_number = captures.fetch_add(1, Ordering::AcqRel);
            if capture_number >= 64 {
                return Err(Failure::new(
                    "limit_exceeded",
                    "Session capture limit is 64 images; start a new session",
                ));
            }
            let (image, frame, scale) = cx
                .update_window(handle, |_, window, cx| {
                    if window.pilot_needs_draw() {
                        window.draw(cx).clear(cx);
                    }
                    let snapshot = snapshot(window, session)?;
                    if snapshot.width * snapshot.height * snapshot.scale * snapshot.scale
                        > 16_000_000.
                    {
                        return Err(Failure::new(
                            "limit_exceeded",
                            "Capture exceeds 16 million pixels",
                        ));
                    }
                    // Secret inputs are omitted at source; refuse all pixels while any is mounted.
                    if snapshot.nodes.iter().any(|n| n.role == "PasswordInput") {
                        return Err(Failure::new(
                            "sensitive_surface",
                            "Capture refused while a password field is mounted",
                        ));
                    }
                    Ok((
                        window.render_to_image().map_err(failure)?,
                        snapshot.frame,
                        snapshot.scale,
                    ))
                })
                .map_err(|_| Failure::new("window_closed", "Selected window is closed"))??;
            let (width, height) = image.dimensions();
            let path = session
                .directory
                .join(format!("capture-{}.png", capture_number + 1));
            let output_path = path.to_string_lossy().into_owned();
            cx.background_executor()
                .spawn(async move { image.save(&path) })
                .await
                .map_err(failure)?;
            Ok(Output::Screenshot {
                path: output_path,
                frame,
                window: session.manifest.window.clone(),
                width,
                height,
                scale,
            })
        }
        command => {
            let snapshot = cx
                .update_window(handle, |_, window, cx| {
                    if window.pilot_needs_draw() {
                        window.draw(cx).clear(cx);
                    }
                    let before = snapshot(window, session)?;
                    match command {
                        Command::Click { reference } => {
                            let node = before.resolve(reference)?;
                            if !node.enabled {
                                return Err(Failure::new("disabled", "Target is disabled"));
                            }
                            if !node.clickable && !node.editable {
                                return Err(Failure::new(
                                    "unsupported_capability",
                                    "Target has no input action",
                                ));
                            }
                            let id = node_id(reference)?;
                            let bounds = window.pilot_hit_bounds(id).ok_or_else(|| {
                                Failure::new("target_occluded", "Target has no hitbox")
                            })?;
                            let position = bounds.center();
                            if !window.pilot_hit_test(id, position) {
                                return Err(Failure::new(
                                    "target_occluded",
                                    "Target center does not receive pointer input",
                                ));
                            }
                            window.dispatch_event(
                                PlatformInput::MouseMove(MouseMoveEvent {
                                    position,
                                    pressed_button: None,
                                    modifiers: Modifiers::default(),
                                }),
                                cx,
                            );
                            window.dispatch_event(
                                PlatformInput::MouseDown(MouseDownEvent {
                                    position,
                                    button: MouseButton::Left,
                                    modifiers: Modifiers::default(),
                                    click_count: 1,
                                    first_mouse: false,
                                }),
                                cx,
                            );
                            window.dispatch_event(
                                PlatformInput::MouseUp(MouseUpEvent {
                                    position,
                                    button: MouseButton::Left,
                                    modifiers: Modifiers::default(),
                                    click_count: 1,
                                }),
                                cx,
                            );
                        }
                        Command::Press { key } => {
                            let key = Keystroke::parse(key).map_err(|_| {
                                Failure::new("invalid_key", "GPUI could not parse key")
                            })?;
                            window.dispatch_keystroke(key.clone(), cx);
                            window.dispatch_event(
                                PlatformInput::KeyUp(KeyUpEvent { keystroke: key }),
                                cx,
                            );
                        }
                        Command::Type { reference, text } => {
                            let node = before.resolve(reference)?;
                            if !node.editable || !node.enabled {
                                return Err(Failure::new("not_editable", "Target is not editable"));
                            }
                            if !node.focused {
                                return Err(Failure::new(
                                    "not_focused",
                                    "Click the input before typing",
                                ));
                            }
                            if !window.pilot_type_text(text, cx) {
                                return Err(Failure::new(
                                    "not_editable",
                                    "No active GPUI input handler",
                                ));
                            }
                        }
                        _ => unreachable!(),
                    }
                    // This is a synchronous GPUI commit, including deferred overlays and input handlers.
                    // The returned frame acknowledges dispatch; only a predicate proves the outcome.
                    window.refresh();
                    window.draw(cx).clear(cx);
                    snapshot(window, session)
                })
                .map_err(|_| Failure::new("window_closed", "Selected window is closed"))??;
            Ok(Output::Acted {
                path: "gpui-input".into(),
                snapshot,
            })
        }
    }
}
fn node_id(reference: &str) -> Result<accesskit::NodeId, Failure> {
    reference
        .rsplit('/')
        .next()
        .and_then(|id| id.parse().ok())
        .map(accesskit::NodeId)
        .ok_or_else(|| Failure::new("stale_ref", "Invalid reference"))
}
fn snapshot(window: &Window, session: &Session) -> Result<Snapshot, Failure> {
    let (frame, tree) = window
        .pilot_frame()
        .ok_or_else(|| Failure::new("no_frame", "No committed semantic frame"))?;
    if tree.nodes.len() > 4096 {
        return Err(Failure::new(
            "limit_exceeded",
            "Semantic tree exceeds 4096 nodes",
        ));
    }
    let reference = |id: accesskit::NodeId| {
        format!(
            "@{}/{}/{frame}/{}",
            session.manifest.session, session.manifest.window, id.0
        )
    };
    let parents: HashMap<_, _> = tree
        .nodes
        .iter()
        .flat_map(|(id, node)| node.children().iter().map(move |child| (*child, *id)))
        .collect();
    let mut author_ids = HashSet::new();
    let scale = window.scale_factor();
    let nodes = tree
        .nodes
        .iter()
        .map(|(id, node)| {
            if let Some(author_id) = node.author_id()
                && !author_ids.insert(author_id)
            {
                return Err(Failure::new(
                    "ambiguous_target",
                    format!("Duplicate author ID: {author_id}"),
                ));
            }
            let secret = node.role() == accesskit::Role::PasswordInput;
            let bounds = window
                .pilot_hit_bounds(*id)
                .map(|b| {
                    [
                        f32::from(b.origin.x),
                        f32::from(b.origin.y),
                        f32::from(b.size.width),
                        f32::from(b.size.height),
                    ]
                })
                .or_else(|| {
                    node.bounds().map(|b| {
                        [
                            b.x0 as f32 / scale,
                            b.y0 as f32 / scale,
                            b.width() as f32 / scale,
                            b.height() as f32 / scale,
                        ]
                    })
                });
            let editable = matches!(
                node.role(),
                accesskit::Role::TextInput | accesskit::Role::MultilineTextInput
            ) && !node.is_read_only();
            Ok(Node {
                reference: reference(*id),
                parent: parents.get(id).copied().map(reference),
                author_id: node.author_id().map(str::to_owned),
                role: format!("{:?}", node.role()),
                name: node.label().map(str::to_owned),
                value: (!secret).then(|| node.value().map(str::to_owned)).flatten(),
                bounds,
                visible: !node.is_hidden() && bounds.is_some_and(|[_, _, w, h]| w > 0. && h > 0.),
                enabled: !node.is_disabled(),
                focused: tree.focus == *id,
                selected: node.is_selected(),
                checked: node.toggled().map(|v| v == accesskit::Toggled::True),
                clickable: node.supports_action(accesskit::Action::Click),
                editable,
            })
        })
        .collect::<Result<Vec<_>, Failure>>()?;
    Ok(Snapshot {
        session: session.manifest.session.clone(),
        window: session.manifest.window.clone(),
        title: session.manifest.title.clone(),
        frame,
        width: f32::from(window.viewport_size().width),
        height: f32::from(window.viewport_size().height),
        scale,
        nodes,
    })
}
