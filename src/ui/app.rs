use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use iocraft::prelude::*;

use crate::app::{App, ChatMessage, Screen, SlashResult};
use crate::config::Config;
use crate::starflask::{self, ProgressEvent, SessionEvent, StarflaskClient};

use super::command_hint::{self, CommandHint};
use super::header::HeaderBar;
use super::input::InputBar;
use super::messages::MessageList;
use super::spinner::SpinnerRow;

/// Truncate a string to at most `max` characters (char-boundary safe).
fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max).collect();
        format!("{}...", truncated)
    }
}

/// Shared progress text updated by background tasks, polled by UI.
type SharedProgress = Arc<StdMutex<Option<String>>>;

/// Shared queue for incoming WebSocket session events.
type SharedEvents = Arc<StdMutex<Vec<SessionEvent>>>;

/// Async API commands that need network calls.
enum AsyncCmd {
    Tasks { project_id: String, status_filter: Option<String> },
    TaskCreate { project_id: String, title: String, description: String, priority: String },
    TaskUpdate { project_id: String, task_id: String, status: String },
    Schedules { agent_id: String },
    Credits,
    History { project_id: String, limit: u32 },
    Memories { agent_id: String, limit: u32 },
}

#[derive(Default, Props)]
pub struct StarkbotAppProps {
    pub config: Option<Config>,
}

#[component]
pub fn StarkbotApp(props: &mut StarkbotAppProps, mut hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let (width, height) = hooks.use_terminal_size();
    let mut system = hooks.use_context_mut::<SystemContext>();

    // Core UI state
    let mut messages = hooks.use_state(|| Vec::<ChatMessage>::new());
    let mut input_value = hooks.use_state(|| String::new());
    let mut querying = hooks.use_state(|| false);
    let mut progress_text = hooks.use_state(|| String::new());
    let mut current_agent = hooks.use_state(|| String::from("general"));
    let mut current_agent_id = hooks.use_state(|| Option::<String>::None);
    let mut connected = hooks.use_state(|| false);
    let mut screen = hooks.use_state(|| Screen::Chat);
    let mut project_id_state = hooks.use_state(|| Option::<String>::None);
    let mut should_exit = hooks.use_state(|| false);
    let mut credits = hooks.use_state(|| Option::<i64>::None);
    let mut selected_hint = hooks.use_state(|| 0usize);

    // Shared app state behind Arc<StdMutex> (thread-safe)
    let mut app_arc: State<Option<Arc<StdMutex<App>>>> = hooks.use_state(|| None);

    // Shared progress for background tasks to write into
    let shared_progress: State<SharedProgress> =
        hooks.use_state(|| Arc::new(StdMutex::new(None)));

    // Initialize on first render
    let mut initialized = hooks.use_state(|| false);
    if !initialized.get() {
        if let Some(config) = props.config.take() {
            let has_client = config.api_key().is_some();
            let pid = config.project_id.clone();
            let app = App::new(config);

            current_agent.set(app.current_agent.clone());
            current_agent_id.set(app.current_agent_id.clone());
            connected.set(has_client);
            project_id_state.set(pid);

            if !has_client {
                screen.set(Screen::Setup);
                let mut msgs = messages.read().clone();
                msgs.push(ChatMessage {
                    role: "system".into(),
                    content: "\n\
       .    *        .       *    .        *     .    \n\
    *        .    .        .        .   *       .     \n\
        .       *    .  *      .       .    *         \n\
   .  *     .              .      *          .     *  \n\
      .        .  *   s t a r k   b o t   .        . \n\
   *       .         .       *        .    *    .     \n\
     .   *      .  *     .       .  *        .        \n\
        .    .        *      .         *   .      *   \n\
   .        *    .        .    *   .        .     .   \n\
\n\
   Welcome! stark-bot connects to Starflask, an AI agent\n\
   orchestration platform. Your agents, tasks, memories,\n\
   and sessions all live on Starflask — this TUI is how\n\
   you talk to them.\n\
\n\
   To get started, paste your Starflask API key below.\n\
   Find it at: https://starflask.com/settings".into(),
                });
                messages.set(msgs);
            }

            app_arc.set(Some(Arc::new(StdMutex::new(app))));
            initialized.set(true);
        }
    }

    // Auto-fetch agents on startup if connected
    {
        let app_arc_clone = app_arc.read().clone();
        let is_connected = connected.get();
        let has_agents = app_arc_clone.as_ref()
            .and_then(|arc| arc.lock().ok().map(|app| !app.agents.is_empty()))
            .unwrap_or(false);
        hooks.use_future(async move {
            if !is_connected || has_agents {
                return;
            }
            if let Some(arc) = app_arc_clone.as_ref() {
                let config = {
                    if let Ok(app) = arc.lock() {
                        Some(app.config.clone())
                    } else {
                        None
                    }
                };
                if let Some(config) = config {
                    let result: Result<Vec<serde_json::Value>, String> = smol::unblock(move || {
                        let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
                        rt.block_on(async {
                            let client = StarflaskClient::new(&config)?;
                            client.list_agents().await
                        })
                    })
                    .await;
                    if let Ok(remote_agents) = result {
                        if let Ok(mut app) = arc.lock() {
                            app.finish_provision(&remote_agents);
                            current_agent.set(app.current_agent.clone());
                            current_agent_id.set(app.current_agent_id.clone());
                        }
                    } else {
                        let mut msgs = messages.read().clone();
                        msgs.push(ChatMessage {
                            role: "system".into(),
                            content: "No agents found. Run /provision to sync from Starflask.".into(),
                        });
                        messages.set(msgs);
                    }
                }
            }
        });
    }

    // Fetch credits on startup (use_future must be called unconditionally)
    {
        let app_arc_clone = app_arc.read().clone();
        let is_connected = connected.get();
        hooks.use_future(async move {
            if !is_connected {
                return;
            }
            if let Some(arc) = app_arc_clone.as_ref() {
                let config = {
                    if let Ok(app) = arc.lock() {
                        Some(app.config.clone())
                    } else {
                        None
                    }
                };
                if let Some(config) = config {
                    let result: Result<i64, String> = smol::unblock(move || {
                        let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
                        rt.block_on(async {
                            let client = StarflaskClient::new(&config)?;
                            let status = client.get_subscription_status().await?;
                            Ok(status.get("credits").and_then(|v| v.as_i64()).unwrap_or(0))
                        })
                    })
                    .await;
                    if let Ok(c) = result {
                        credits.set(Some(c));
                    }
                }
            }
        });
    }

    // WebSocket session streaming — background listener + poller
    // IMPORTANT: all hooks must be called unconditionally (rules of hooks)
    let ws_events: State<SharedEvents> = hooks.use_state(|| Arc::new(StdMutex::new(Vec::new())));
    {
        // Build WS URL if connected with a project (no hooks inside this conditional)
        let ws_url_opt: Option<String> = if connected.get() {
            app_arc.read().as_ref().and_then(|arc| {
                arc.lock().ok().and_then(|app| {
                    app.config.project_id.as_ref().and_then(|pid| {
                        app.client.as_ref().map(|client| client.ws_url(pid))
                    })
                })
            })
        } else {
            None
        };

        // WS listener — always called, conditional logic inside the async block
        let events_queue = ws_events.read().clone();
        hooks.use_future(async move {
            let Some(ws_url) = ws_url_opt else { return };
            let eq = events_queue.clone();
            let eq2 = events_queue;
            let result = starflask::ws_session_stream(&ws_url, move |event| {
                if let Ok(mut guard) = eq.lock() {
                    guard.push(event);
                }
            }).await;
            if let Err(e) = result {
                if let Ok(mut guard) = eq2.lock() {
                    guard.push(SessionEvent {
                        session_id: String::new(),
                        agent_id: String::new(),
                        agent_name: None,
                        project_id: None,
                        status: "error".into(),
                        hook_event: None,
                        result: None,
                        error: Some(format!("WS disconnected: {}", e)),
                        source_session_id: None,
                        source_agent_id: None,
                    });
                }
            }
        });

        // WS event poller — always called, drains events into messages
        let ws_ev = ws_events.read().clone();
        hooks.use_future(async move {
            loop {
                smol::Timer::after(Duration::from_millis(250)).await;
                let events: Vec<SessionEvent> = {
                    if let Ok(mut guard) = ws_ev.lock() {
                        guard.drain(..).collect()
                    } else {
                        Vec::new()
                    }
                };
                if !events.is_empty() {
                    let mut msgs = messages.read().clone();
                    for event in events {
                        if event.status == "error" {
                            if let Some(err) = &event.error {
                                msgs.push(ChatMessage {
                                    role: "error".into(),
                                    content: err.clone(),
                                });
                            }
                        } else {
                            msgs.push(ChatMessage {
                                role: "system".into(),
                                content: format!("session {}", event.summary()),
                            });
                        }
                    }
                    messages.set(msgs);
                }
            }
        });
    }

    // Check exit flag
    if should_exit.get() {
        system.exit();
    }

    // Poll shared progress from background tasks
    {
        let sp = shared_progress.read().clone();
        hooks.use_future(async move {
            loop {
                smol::Timer::after(Duration::from_millis(100)).await;
                if let Ok(mut guard) = sp.lock() {
                    if let Some(text) = guard.take() {
                        progress_text.set(text);
                    }
                }
            }
        });
    }

    // Async submit handler
    let submit = hooks.use_async_handler({
        let app_arc_val = app_arc.read().clone();
        let sp = shared_progress.read().clone();
        move |text: String| {
            let text = text.trim().to_string();
            let app_arc_val = app_arc_val.clone();
            let sp = sp.clone();
            async move {
                if text.is_empty() && *screen.read() != Screen::SetupProject {
                    return;
                }

                if text.starts_with('/') {
                    let exit = handle_slash_async(
                        &text, &app_arc_val, &sp, &mut messages, &mut current_agent,
                        &mut current_agent_id, &mut connected, &mut screen, &mut querying,
                        &mut progress_text,
                    )
                    .await;
                    input_value.set(String::new());
                    if exit {
                        should_exit.set(true);
                    }
                } else if *screen.read() == Screen::Setup {
                    // Step 1: Save API key, provision agents, fetch projects
                    let api_key = text.clone();
                    if let Err(e) = crate::config::Config::save_api_key(&api_key) {
                        let mut msgs = messages.read().clone();
                        msgs.push(ChatMessage {
                            role: "error".into(),
                            content: format!("Failed to save key: {}", e),
                        });
                        messages.set(msgs);
                        input_value.set(String::new());
                        return;
                    }

                    // Reconnect client
                    if let Some(arc) = app_arc_val.as_ref() {
                        if let Ok(mut app) = arc.lock() {
                            app.finish_setup();
                            connected.set(app.client.is_some());
                        }
                    }

                    let mut msgs = messages.read().clone();
                    msgs.push(ChatMessage {
                        role: "system".into(),
                        content: "[1/3] API key saved.".into(),
                    });
                    messages.set(msgs);

                    // Auto-provision agents
                    let config = app_arc_val.as_ref().and_then(|arc| {
                        arc.lock().ok().map(|app| app.config.clone())
                    });

                    if let Some(config) = config {
                        let mut msgs = messages.read().clone();
                        msgs.push(ChatMessage {
                            role: "system".into(),
                            content: "[2/3] Syncing agents from Starflask...".into(),
                        });
                        messages.set(msgs);
                        querying.set(true);
                        progress_text.set("Provisioning...".to_string());

                        let config2 = config.clone();
                        let agent_result: Result<Vec<serde_json::Value>, String> =
                            smol::unblock(move || {
                                let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
                                rt.block_on(async {
                                    let client = StarflaskClient::new(&config2)?;
                                    client.list_agents().await
                                })
                            }).await;

                        let mut msgs = messages.read().clone();
                        match agent_result {
                            Ok(remote_agents) => {
                                if let Some(arc) = app_arc_val.as_ref() {
                                    if let Ok(mut app) = arc.lock() {
                                        let synced = app.finish_provision(&remote_agents);
                                        current_agent.set(app.current_agent.clone());
                                        current_agent_id.set(app.current_agent_id.clone());
                                        if synced.is_empty() {
                                            msgs.push(ChatMessage {
                                                role: "system".into(),
                                                content: "     No agents found on Starflask.".into(),
                                            });
                                        } else {
                                            for name in &synced {
                                                msgs.push(ChatMessage {
                                                    role: "system".into(),
                                                    content: format!("     + {}", name),
                                                });
                                            }
                                            msgs.push(ChatMessage {
                                                role: "system".into(),
                                                content: format!("     Synced {} agent(s).", synced.len()),
                                            });
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                msgs.push(ChatMessage {
                                    role: "error".into(),
                                    content: format!("     Agent sync failed: {}", e),
                                });
                            }
                        }
                        messages.set(msgs);

                        // Fetch projects for step 3
                        progress_text.set("Fetching projects...".to_string());
                        let project_result: Result<Vec<serde_json::Value>, String> =
                            smol::unblock(move || {
                                let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
                                rt.block_on(async {
                                    let client = StarflaskClient::new(&config)?;
                                    client.list_projects().await
                                })
                            }).await;

                        querying.set(false);
                        progress_text.set(String::new());

                        let mut msgs = messages.read().clone();
                        match project_result {
                            Ok(projects) if !projects.is_empty() => {
                                msgs.push(ChatMessage {
                                    role: "system".into(),
                                    content: "[3/3] Select a project:".into(),
                                });
                                // Store projects for selection
                                let mut project_list = Vec::new();
                                for (i, p) in projects.iter().enumerate() {
                                    let id = p.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    let name = p.get("name").and_then(|v| v.as_str()).unwrap_or("(unnamed)").to_string();
                                    let short_id = if id.len() > 8 { &id[..8] } else { &id };
                                    msgs.push(ChatMessage {
                                        role: "system".into(),
                                        content: format!("     {}. {} ({})", i + 1, name, short_id),
                                    });
                                    project_list.push((id, name));
                                }
                                msgs.push(ChatMessage {
                                    role: "system".into(),
                                    content: format!("\n     Enter 1-{} to pick, or press Enter to skip.", projects.len()),
                                });
                                // Store project list in a state for the next step
                                if let Some(arc) = app_arc_val.as_ref() {
                                    if let Ok(mut app) = arc.lock() {
                                        app.setup_projects = Some(project_list);
                                    }
                                }
                                screen.set(Screen::SetupProject);
                            }
                            Ok(_) => {
                                msgs.push(ChatMessage {
                                    role: "system".into(),
                                    content: "[3/3] No projects found — using direct agent queries.".into(),
                                });
                                msgs.push(ChatMessage {
                                    role: "system".into(),
                                    content: "\nSetup complete! Start chatting.".into(),
                                });
                                screen.set(Screen::Chat);
                            }
                            Err(e) => {
                                msgs.push(ChatMessage {
                                    role: "system".into(),
                                    content: format!("[3/3] Could not fetch projects: {}", e),
                                });
                                msgs.push(ChatMessage {
                                    role: "system".into(),
                                    content: "\nSetup complete! Start chatting.".into(),
                                });
                                screen.set(Screen::Chat);
                            }
                        }
                        messages.set(msgs);
                    }

                    input_value.set(String::new());
                } else if *screen.read() == Screen::SetupProject {
                    // Step 3: User picks a project by number
                    let choice = text.trim().to_string();
                    let mut msgs = messages.read().clone();

                    let project_list = app_arc_val.as_ref().and_then(|arc| {
                        arc.lock().ok().and_then(|mut app| app.setup_projects.take())
                    });

                    if let Some(projects) = project_list {
                        if let Ok(n) = choice.parse::<usize>() {
                            if n >= 1 && n <= projects.len() {
                                let (pid, pname) = &projects[n - 1];
                                if let Some(arc) = app_arc_val.as_ref() {
                                    if let Ok(mut app) = arc.lock() {
                                        app.config.project_id = Some(pid.clone());
                                        let _ = app.config.save();
                                    }
                                }
                                project_id_state.set(Some(pid.clone()));
                                msgs.push(ChatMessage {
                                    role: "system".into(),
                                    content: format!("Project set to: {}", pname),
                                });
                            } else {
                                msgs.push(ChatMessage {
                                    role: "error".into(),
                                    content: format!("Invalid choice. Enter 1-{}.", projects.len()),
                                });
                                // Put projects back so they can retry
                                if let Some(arc) = app_arc_val.as_ref() {
                                    if let Ok(mut app) = arc.lock() {
                                        app.setup_projects = Some(projects);
                                    }
                                }
                                messages.set(msgs);
                                input_value.set(String::new());
                                return;
                            }
                        } else {
                            msgs.push(ChatMessage {
                                role: "system".into(),
                                content: "Skipped — using direct agent queries.".into(),
                            });
                        }
                    }

                    msgs.push(ChatMessage {
                        role: "system".into(),
                        content: "\nSetup complete! Start chatting.".into(),
                    });
                    messages.set(msgs);
                    screen.set(Screen::Chat);
                    input_value.set(String::new());
                } else {
                    // Normal query
                    let mut msgs = messages.read().clone();
                    msgs.push(ChatMessage {
                        role: "user".into(),
                        content: text.clone(),
                    });
                    messages.set(msgs);
                    input_value.set(String::new());
                    querying.set(true);
                    progress_text.set("Sending query...".to_string());

                    run_query(
                        &text, &app_arc_val, &sp, &mut messages, &mut querying,
                        &mut progress_text,
                    )
                    .await;
                }
            }
        }
    });

    // Compute slash command suggestions for autocomplete popup
    let suggestions = {
        let input_text = input_value.read().clone();
        if input_text.starts_with('/') && !input_text.contains(' ') {
            command_hint::filter_commands(&input_text)
        } else {
            vec![]
        }
    };
    let hint_count = suggestions.len();
    let hint_idx = if hint_count == 0 {
        0
    } else {
        selected_hint.get().min(hint_count - 1)
    };

    // Build the commands data for the popup (before suggestions is moved)
    let hint_commands: Vec<(&'static str, &'static str, &'static str)> = suggestions
        .iter()
        .map(|c| (c.name, c.args, c.desc))
        .collect();

    // Keyboard events: Ctrl+C to quit, Enter to submit, Tab/arrows for hints
    hooks.use_terminal_events({
        move |event| match event {
            TerminalEvent::Key(KeyEvent { code, modifiers, kind, .. })
                if kind != KeyEventKind::Release =>
            {
                if modifiers.contains(KeyModifiers::CONTROL) && code == KeyCode::Char('c') {
                    should_exit.set(true);
                } else if code == KeyCode::Tab && hint_count > 0 {
                    // Complete selected command
                    let cmds = command_hint::filter_commands(&input_value.read());
                    if let Some(cmd) = cmds.get(selected_hint.get().min(cmds.len().saturating_sub(1))) {
                        let completed = if cmd.args.is_empty() {
                            cmd.name.to_string()
                        } else {
                            format!("{} ", cmd.name)
                        };
                        input_value.set(completed);
                        selected_hint.set(0);
                    }
                } else if code == KeyCode::Up && hint_count > 0 {
                    let idx = selected_hint.get();
                    selected_hint.set(if idx == 0 { hint_count - 1 } else { idx - 1 });
                } else if code == KeyCode::Down && hint_count > 0 {
                    let idx = selected_hint.get();
                    selected_hint.set(if idx >= hint_count - 1 { 0 } else { idx + 1 });
                } else if code == KeyCode::Enter {
                    // Read current value from state directly (not a stale render-time
                    // snapshot) so pasted text is captured even before re-render.
                    let val = input_value.read().clone();
                    if !val.trim().is_empty() {
                        submit(val);
                    } else if *screen.read() == Screen::SetupProject {
                        // Allow empty Enter to skip project selection
                        submit(String::new());
                    }
                }
            }
            _ => {}
        }
    });

    let on_input_change = move |new_value: String| {
        input_value.set(new_value);
        selected_hint.set(0);
    };

    let is_querying = querying.get();
    let progress = progress_text.read().clone();
    let current_screen = *screen.read();
    let is_setup = current_screen == Screen::Setup;

    let prompt = match current_screen {
        Screen::Setup => "key> ",
        Screen::SetupProject => "  #> ",
        Screen::Chat => "you> ",
    };
    let display_value = if is_setup {
        "*".repeat(input_value.read().len())
    } else {
        input_value.read().clone()
    };

    element! {
        View(
            width,
            height,
            flex_direction: FlexDirection::Column,
        ) {
            HeaderBar(
                agent: current_agent.read().clone(),
                connected: connected.get(),
                project: project_id_state.read().clone(),
                credits: *credits.read(),
            )
            View(flex_grow: 1.0, overflow: Overflow::Hidden) {
                MessageList(
                    messages: messages.read().clone(),
                    current_agent: current_agent.read().clone(),
                )
            }
            #(if is_querying {
                Some(element! {
                    SpinnerRow(text: progress)
                })
            } else {
                None
            })
            InputBar(prompt: prompt) {
                TextInput(
                    has_focus: true,
                    value: display_value,
                    on_change: on_input_change,
                )
            }
            #(if !hint_commands.is_empty() {
                Some(element! {
                    View(
                        position: Position::Absolute,
                        bottom: 3,
                        left: 1,
                        width: width.saturating_sub(2),
                    ) {
                        CommandHint(
                            commands: hint_commands.clone(),
                            selected: hint_idx,
                            width: width.saturating_sub(4),
                        )
                    }
                })
            } else {
                None
            })
        }
    }
}

/// Run a query against the API.
async fn run_query(
    text: &str,
    app_arc: &Option<Arc<StdMutex<App>>>,
    sp: &SharedProgress,
    messages: &mut State<Vec<ChatMessage>>,
    querying: &mut State<bool>,
    progress_text: &mut State<String>,
) {
    let (config, project_id, agent_id_val) = {
        if let Some(arc) = app_arc.as_ref() {
            if let Ok(app) = arc.lock() {
                (
                    Some(app.config.clone()),
                    app.config.project_id.clone(),
                    app.current_agent_id.clone(),
                )
            } else {
                (None, None, None)
            }
        } else {
            (None, None, None)
        }
    };

    if let Some(config) = config {
        let msg = text.to_string();
        let sp2 = sp.clone();

        let result: Result<String, String> = smol::unblock(move || {
            let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
            rt.block_on(async {
                let client = StarflaskClient::new(&config)?;

                if let Some(pid) = project_id {
                    let (session_id, _) = client.project_query(&pid, &msg).await?;
                    let sp3 = sp2.clone();
                    let session = client
                        .poll_session_by_id(&session_id, move |evt| {
                            let text = match evt {
                                ProgressEvent::StatusChange(s) => format!("Status: {}", s),
                                ProgressEvent::LogEntry { summary, .. } => summary,
                                ProgressEvent::Error(e) => format!("Warning: {}", e),
                            };
                            if let Ok(mut g) = sp3.lock() {
                                *g = Some(text);
                            }
                        })
                        .await?;
                    Ok(starflask::parse_text_result(&session.result))
                } else {
                    let agent_id = agent_id_val.ok_or_else(|| {
                        "No project or agent configured. Run `stark-bot setup`.".to_string()
                    })?;
                    let session_id = client.create_session(&agent_id, &msg).await?;
                    let sp3 = sp2.clone();
                    let session = client
                        .poll_session(&agent_id, &session_id, move |evt| {
                            let text = match evt {
                                ProgressEvent::StatusChange(s) => format!("Status: {}", s),
                                ProgressEvent::LogEntry { summary, .. } => summary,
                                ProgressEvent::Error(e) => format!("Warning: {}", e),
                            };
                            if let Ok(mut g) = sp3.lock() {
                                *g = Some(text);
                            }
                        })
                        .await?;
                    Ok(starflask::parse_text_result(&session.result))
                }
            })
        })
        .await;

        querying.set(false);
        progress_text.set(String::new());

        let mut msgs = messages.read().clone();
        match result {
            Ok(text) => {
                let content = if text.is_empty() {
                    "(empty response)".to_string()
                } else {
                    text
                };
                msgs.push(ChatMessage {
                    role: "agent".into(),
                    content,
                });
            }
            Err(e) => {
                msgs.push(ChatMessage {
                    role: "error".into(),
                    content: format!("Error: {}", e),
                });
            }
        }
        messages.set(msgs);
    } else {
        querying.set(false);
        progress_text.set(String::new());
        let mut msgs = messages.read().clone();
        msgs.push(ChatMessage {
            role: "error".into(),
            content: "Not connected. Use /connect to set API key.".into(),
        });
        messages.set(msgs);
    }
}

/// Handle slash commands. Returns true if the app should exit.
async fn handle_slash_async(
    text: &str,
    app_arc: &Option<Arc<StdMutex<App>>>,
    _sp: &SharedProgress,
    messages: &mut State<Vec<ChatMessage>>,
    current_agent: &mut State<String>,
    current_agent_id: &mut State<Option<String>>,
    _connected: &mut State<bool>,
    screen: &mut State<Screen>,
    querying: &mut State<bool>,
    progress_text: &mut State<String>,
) -> bool {
    let Some(arc) = app_arc else { return false };

    enum SlashAction {
        Messages(Vec<ChatMessage>),
        Provision { config: Config },
        AsyncApi { config: Config, cmd: AsyncCmd },
        Quit,
    }

    let action = {
        let Ok(mut app) = arc.lock() else { return false };
        let result = app.handle_slash_command(text);
        let mut msgs = Vec::new();

        match result {
            SlashResult::Help(help_text) => {
                msgs.push(ChatMessage {
                    role: "system".into(),
                    content: help_text.replace("\r\n", "\n"),
                });
                SlashAction::Messages(msgs)
            }
            SlashResult::Agents => {
                let agents = app.agents.clone();
                let current = app.current_agent.clone();
                if agents.is_empty() {
                    msgs.push(ChatMessage {
                        role: "system".into(),
                        content: "No agents. Run /provision to sync.".into(),
                    });
                } else {
                    for a in &agents {
                        let marker = if a.capability == current { " *" } else { "" };
                        msgs.push(ChatMessage {
                            role: "system".into(),
                            content: format!("  {} ({}){}", a.capability, a.name, marker),
                        });
                    }
                }
                SlashAction::Messages(msgs)
            }
            SlashResult::Switched(msg) => {
                current_agent.set(app.current_agent.clone());
                current_agent_id.set(app.current_agent_id.clone());
                msgs.push(ChatMessage { role: "system".into(), content: msg });
                SlashAction::Messages(msgs)
            }
            SlashResult::Provision => {
                if app.client.is_none() {
                    msgs.push(ChatMessage {
                        role: "error".into(),
                        content: "Not connected. Use /connect to set API key.".into(),
                    });
                    SlashAction::Messages(msgs)
                } else {
                    SlashAction::Provision { config: app.config.clone() }
                }
            }
            SlashResult::Connect => {
                screen.set(Screen::Setup);
                msgs.push(ChatMessage {
                    role: "system".into(),
                    content: "Enter your Starflask API key:".into(),
                });
                SlashAction::Messages(msgs)
            }
            SlashResult::Reset => {
                if let Err(e) = crate::config::Config::reset() {
                    msgs.push(ChatMessage {
                        role: "error".into(),
                        content: format!("Reset failed: {}", e),
                    });
                } else {
                    app.client = None;
                    app.agents.clear();
                    app.current_agent = "general".into();
                    app.current_agent_id = None;
                    _connected.set(false);
                    current_agent.set("general".into());
                    current_agent_id.set(None);
                    screen.set(Screen::Setup);
                    msgs.push(ChatMessage {
                        role: "system".into(),
                        content: "Config wiped. API key, settings, and agents cleared.\nPaste a new Starflask API key to reconnect.".into(),
                    });
                }
                SlashAction::Messages(msgs)
            }
            SlashResult::Clear => {
                messages.set(Vec::new());
                return false;
            }
            SlashResult::Quit => SlashAction::Quit,
            SlashResult::Unknown(msg) => {
                msgs.push(ChatMessage { role: "error".into(), content: msg });
                SlashAction::Messages(msgs)
            }
            // New async commands
            SlashResult::Tasks { status_filter } => {
                if let Some(pid) = app.config.project_id.clone() {
                    SlashAction::AsyncApi {
                        config: app.config.clone(),
                        cmd: AsyncCmd::Tasks { project_id: pid, status_filter },
                    }
                } else {
                    msgs.push(ChatMessage {
                        role: "error".into(),
                        content: "No project configured. Run `stark-bot setup` to select a project.".into(),
                    });
                    SlashAction::Messages(msgs)
                }
            }
            SlashResult::TaskCreate { title, description, priority } => {
                if let Some(pid) = app.config.project_id.clone() {
                    SlashAction::AsyncApi {
                        config: app.config.clone(),
                        cmd: AsyncCmd::TaskCreate { project_id: pid, title, description, priority },
                    }
                } else {
                    msgs.push(ChatMessage {
                        role: "error".into(),
                        content: "No project configured. Run `stark-bot setup`.".into(),
                    });
                    SlashAction::Messages(msgs)
                }
            }
            SlashResult::TaskUpdate { task_id, status } => {
                if let Some(pid) = app.config.project_id.clone() {
                    SlashAction::AsyncApi {
                        config: app.config.clone(),
                        cmd: AsyncCmd::TaskUpdate { project_id: pid, task_id, status },
                    }
                } else {
                    msgs.push(ChatMessage {
                        role: "error".into(),
                        content: "No project configured.".into(),
                    });
                    SlashAction::Messages(msgs)
                }
            }
            SlashResult::Schedules => {
                if let Some(aid) = app.current_agent_id.clone() {
                    SlashAction::AsyncApi {
                        config: app.config.clone(),
                        cmd: AsyncCmd::Schedules { agent_id: aid },
                    }
                } else {
                    msgs.push(ChatMessage {
                        role: "error".into(),
                        content: "No agent selected.".into(),
                    });
                    SlashAction::Messages(msgs)
                }
            }
            SlashResult::Credits => {
                SlashAction::AsyncApi {
                    config: app.config.clone(),
                    cmd: AsyncCmd::Credits,
                }
            }
            SlashResult::History { limit } => {
                if let Some(pid) = app.config.project_id.clone() {
                    SlashAction::AsyncApi {
                        config: app.config.clone(),
                        cmd: AsyncCmd::History { project_id: pid, limit },
                    }
                } else {
                    msgs.push(ChatMessage {
                        role: "error".into(),
                        content: "No project configured. Run `stark-bot setup`.".into(),
                    });
                    SlashAction::Messages(msgs)
                }
            }
            SlashResult::Memories { limit } => {
                if let Some(aid) = app.current_agent_id.clone() {
                    SlashAction::AsyncApi {
                        config: app.config.clone(),
                        cmd: AsyncCmd::Memories { agent_id: aid, limit },
                    }
                } else {
                    msgs.push(ChatMessage {
                        role: "error".into(),
                        content: "No agent selected.".into(),
                    });
                    SlashAction::Messages(msgs)
                }
            }
        }
    }; // lock dropped here

    match action {
        SlashAction::Quit => return true,
        SlashAction::Messages(new_msgs) => {
            let mut msgs = messages.read().clone();
            msgs.extend(new_msgs);
            messages.set(msgs);
        }
        SlashAction::Provision { config } => {
            let mut msgs = messages.read().clone();
            msgs.push(ChatMessage {
                role: "system".into(),
                content: "Syncing agents from Starflask...".into(),
            });
            messages.set(msgs);

            querying.set(true);
            progress_text.set("Provisioning...".to_string());

            let result: Result<Vec<serde_json::Value>, String> =
                smol::unblock(move || {
                    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
                    rt.block_on(async {
                        let client = StarflaskClient::new(&config)?;
                        client.list_agents().await
                    })
                })
                .await;

            querying.set(false);
            progress_text.set(String::new());

            let mut msgs = messages.read().clone();
            match result {
                Ok(remote_agents) => {
                    if let Ok(mut app) = arc.lock() {
                        let synced = app.finish_provision(&remote_agents);
                        current_agent.set(app.current_agent.clone());
                        current_agent_id.set(app.current_agent_id.clone());
                        if synced.is_empty() {
                            msgs.push(ChatMessage {
                                role: "system".into(),
                                content: "No agents found on Starflask.".into(),
                            });
                        } else {
                            for name in &synced {
                                msgs.push(ChatMessage {
                                    role: "system".into(),
                                    content: format!("  {}", name),
                                });
                            }
                            msgs.push(ChatMessage {
                                role: "system".into(),
                                content: format!("Synced {} agent(s).", synced.len()),
                            });
                        }
                    }
                }
                Err(e) => {
                    msgs.push(ChatMessage {
                        role: "error".into(),
                        content: format!("Provision failed: {}", e),
                    });
                }
            }
            messages.set(msgs);
        }
        SlashAction::AsyncApi { config, cmd } => {
            querying.set(true);
            progress_text.set("Loading...".to_string());

            let result: Result<Vec<ChatMessage>, String> = smol::unblock(move || {
                let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
                rt.block_on(async { run_async_cmd(&config, cmd).await })
            })
            .await;

            querying.set(false);
            progress_text.set(String::new());

            let mut msgs = messages.read().clone();
            match result {
                Ok(new_msgs) => msgs.extend(new_msgs),
                Err(e) => {
                    msgs.push(ChatMessage {
                        role: "error".into(),
                        content: format!("Error: {}", e),
                    });
                }
            }
            messages.set(msgs);
        }
    }

    false
}

/// Execute an async API command and return messages to display.
async fn run_async_cmd(
    config: &Config,
    cmd: AsyncCmd,
) -> Result<Vec<ChatMessage>, String> {
    let client = StarflaskClient::new(config)?;
    let mut msgs = Vec::new();

    match cmd {
        AsyncCmd::Tasks { project_id, status_filter } => {
            let tasks = client
                .list_project_tasks(&project_id, status_filter.as_deref())
                .await?;
            if tasks.is_empty() {
                msgs.push(ChatMessage {
                    role: "system".into(),
                    content: "No tasks found.".into(),
                });
            } else {
                msgs.push(ChatMessage {
                    role: "system".into(),
                    content: format!("Tasks ({}):", tasks.len()),
                });
                for t in &tasks {
                    let id = t.get("id").and_then(|v| v.as_str()).unwrap_or("?");
                    let short_id = &id[..8.min(id.len())];
                    let title = t.get("title").and_then(|v| v.as_str()).unwrap_or("(untitled)");
                    let status = t.get("status").and_then(|v| v.as_str()).unwrap_or("?");
                    let priority = t.get("priority").and_then(|v| v.as_str()).unwrap_or("?");
                    let agent = t.get("assigned_agent_id")
                        .and_then(|v| v.as_str())
                        .map(|a| format!(" -> {}", &a[..8.min(a.len())]))
                        .unwrap_or_default();
                    let status_icon = match status {
                        "done" => "[x]",
                        "in_progress" => "[~]",
                        "blocked" => "[!]",
                        _ => "[ ]",
                    };
                    msgs.push(ChatMessage {
                        role: "system".into(),
                        content: format!(
                            "  {} {} [{}] {}{} ({})",
                            status_icon, short_id, priority, title, agent, status
                        ),
                    });
                }
            }
        }
        AsyncCmd::TaskCreate { project_id, title, description, priority } => {
            let task = client
                .create_project_task(&project_id, &title, &description, &priority)
                .await?;
            let id = task.get("id").and_then(|v| v.as_str()).unwrap_or("?");
            msgs.push(ChatMessage {
                role: "system".into(),
                content: format!("Task created: {} ({})", title, &id[..8.min(id.len())]),
            });
        }
        AsyncCmd::TaskUpdate { project_id, task_id, status } => {
            client
                .update_project_task_status(&project_id, &task_id, &status)
                .await?;
            msgs.push(ChatMessage {
                role: "system".into(),
                content: format!("Task {} updated to: {}", &task_id[..8.min(task_id.len())], status),
            });
        }
        AsyncCmd::Schedules { agent_id } => {
            let tasks = client.list_agent_tasks(&agent_id).await?;
            if tasks.is_empty() {
                msgs.push(ChatMessage {
                    role: "system".into(),
                    content: "No schedules found for this agent.".into(),
                });
            } else {
                msgs.push(ChatMessage {
                    role: "system".into(),
                    content: format!("Agent schedules ({}):", tasks.len()),
                });
                for t in &tasks {
                    let hook = t.get("hook_event").and_then(|v| v.as_str()).unwrap_or("?");
                    let schedule = t.get("schedule").and_then(|v| v.as_str()).unwrap_or("manual");
                    let prompt = t.get("prompt").and_then(|v| v.as_str()).unwrap_or("");
                    let prompt_preview = if prompt.len() > 60 {
                        format!("{}...", &prompt[..60])
                    } else {
                        prompt.to_string()
                    };
                    msgs.push(ChatMessage {
                        role: "system".into(),
                        content: format!("  [{}] {} | {}", schedule, hook, prompt_preview),
                    });
                }
            }
        }
        AsyncCmd::Credits => {
            let status = client.get_subscription_status().await?;
            let credits = status.get("credits").and_then(|v| v.as_i64()).unwrap_or(0);
            let plan_status = status.get("status").and_then(|v| v.as_str()).unwrap_or("none");
            let is_active = status.get("is_active").and_then(|v| v.as_bool()).unwrap_or(false);

            let status_str = if is_active {
                format!("{} (active)", plan_status)
            } else {
                format!("{} (inactive)", plan_status)
            };

            msgs.push(ChatMessage {
                role: "system".into(),
                content: format!("Credits: {} | Subscription: {}", credits, status_str),
            });
        }
        AsyncCmd::History { project_id, limit } => {
            let sessions = client.list_project_sessions(&project_id, limit).await?;
            if sessions.is_empty() {
                msgs.push(ChatMessage {
                    role: "system".into(),
                    content: "No recent sessions.".into(),
                });
            } else {
                msgs.push(ChatMessage {
                    role: "system".into(),
                    content: format!("Recent sessions ({}):", sessions.len()),
                });
                for s in &sessions {
                    let id = s.get("id").and_then(|v| v.as_str()).unwrap_or("?");
                    let short_id = &id[..8.min(id.len())];
                    let agent = s.get("agent_name").and_then(|v| v.as_str()).unwrap_or("?");
                    let status = s.get("status").and_then(|v| v.as_str()).unwrap_or("?");
                    let hook = s.get("hook_event").and_then(|v| v.as_str()).unwrap_or("query");
                    let summary = s.get("result_summary")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let summary_preview = truncate_chars(summary, 50);
                    let status_icon = match status {
                        "completed" => "ok",
                        "failed" => "FAIL",
                        _ => status,
                    };
                    msgs.push(ChatMessage {
                        role: "system".into(),
                        content: format!(
                            "  {} [{}] {} ({}) {}",
                            short_id, status_icon, agent, hook, summary_preview
                        ),
                    });
                }
            }
        }
        AsyncCmd::Memories { agent_id, limit } => {
            let memories = client.list_agent_memories(&agent_id, limit).await?;
            if memories.is_empty() {
                msgs.push(ChatMessage {
                    role: "system".into(),
                    content: "No memories found for this agent.".into(),
                });
            } else {
                msgs.push(ChatMessage {
                    role: "system".into(),
                    content: format!("Agent memories ({}):", memories.len()),
                });
                for m in &memories {
                    let content = m.get("content").and_then(|v| v.as_str()).unwrap_or("?");
                    let mem_type = m.get("memory_type").and_then(|v| v.as_str()).unwrap_or("?");
                    let importance = m.get("importance").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let tags = m.get("tags")
                        .and_then(|v| v.as_array())
                        .map(|arr| arr.iter()
                            .filter_map(|t| t.as_str())
                            .collect::<Vec<_>>()
                            .join(", "))
                        .unwrap_or_default();
                    let content_preview = truncate_chars(content, 80);
                    let tag_str = if tags.is_empty() {
                        String::new()
                    } else {
                        format!(" [{}]", tags)
                    };
                    msgs.push(ChatMessage {
                        role: "system".into(),
                        content: format!(
                            "  [{}] ({:.1}) {}{}",
                            mem_type, importance, content_preview, tag_str
                        ),
                    });
                }
            }
        }
    }

    Ok(msgs)
}

