// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(target_os = "macos")]
mod mac;
#[cfg(target_os = "windows")]
mod windows;

mod traits;

use easier::prelude::*;
use std::{
    collections::HashSet,
    sync::{
        mpsc::{Receiver, Sender},
        Mutex,
    },
    time::Duration,
};

use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use serde::{Deserialize, Serialize};
use tauri::{
    AppHandle, CustomMenuItem, Manager, PhysicalPosition, PhysicalSize, Position, Size, State,
    SystemTray, SystemTrayEvent, SystemTrayMenu, Window,
};
use traits::{AccessibilityCalls, Action, UiElement};

struct AppState {
    input: String,
    results: Vec<String>,
    sender: Sender<Message>,
}

fn main() {
    println!("starting");

    let tray = setup_system_tray();
    let debug = false;
    let show_taskbar = true;
    let (sender, rec) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        worker(rec, debug, show_taskbar);
    });
    let state = AppState {
        input: "".to_string(),
        results: vec![],
        sender,
    };
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![update_input, choice, hide, show])
        .manage(Mutex::new(state))
        .system_tray(tray)
        .on_system_tray_event(handle_system_tray) // <- handling the system tray events
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory); //dont show in dock
                                                                           //listen to get fullscreen
            let ah = app.app_handle();
            app.listen_global("go_full", move |_event| {
                let window = ah.get_focused_window().unwrap();
                set_full_size(&window);
            });

            let state: State<Mutex<AppState>> = app.state();
            let window = app.get_window("main").unwrap();
            set_output_size(&window);

            let state = state.lock().unwrap();
            state
                .sender
                .send(Message::AppHandle(app.app_handle()))
                .unwrap();

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn setup_system_tray() -> SystemTray {
    let quit = CustomMenuItem::new("quit".to_string(), "Quit");
    let tray_menu = SystemTrayMenu::new().add_item(quit);
    SystemTray::new().with_menu(tray_menu)
}

fn handle_system_tray(app: &tauri::AppHandle, event: tauri::SystemTrayEvent) {
    match event {
        SystemTrayEvent::LeftClick { .. } => toggle_window(app),
        SystemTrayEvent::MenuItemClick { id, .. } => match id.as_str() {
            "quit" => app.exit(0),
            "toggle" => toggle_window(app),
            _ => {}
        },
        _ => {}
    }
}

fn toggle_window(app: &tauri::AppHandle) {
    let window = app.get_window("main").unwrap();
    if window.is_visible().unwrap() {
        hide_window(app.clone());
    } else {
        show_window(app.clone());
        window.unminimize().unwrap();
        window.set_focus().unwrap();
    }
}
#[tauri::command]
fn update_input(input: &str, state: tauri::State<Mutex<AppState>>) {
    state.lock().unwrap().input = input.to_string();
    state.lock().unwrap().results = vec![];
    let res = state
        .lock()
        .unwrap()
        .sender
        .send(Message::UpdateInput(input.to_string()));
    if let Err(e) = res {
        eprintln!("error sending message: {:?}", e);
    }
}

impl From<&str> for Action {
    fn from(s: &str) -> Self {
        match s {
            "LeftClick" => Action::LeftClick,
            "RightClick" => Action::RightClick,
            _ => Action::LeftClick,
        }
    }
}

#[tauri::command]
fn choice(choice: &str, action: &str, state: tauri::State<Mutex<AppState>>, app: AppHandle) {
    hide_window(app);
    std::thread::sleep(Duration::from_millis(100)); //wait to hide window
    let state = state.lock().unwrap();

    println!("choice:{choice}");
    let res = state
        .sender
        .send(Message::Invoke(choice.to_string(), action.into()));
    if let Err(e) = res {
        eprintln!("error sending message: {:?}", e);
    }
}

#[tauri::command]
fn hide(app: AppHandle) {
    hide_window(app);
}

#[tauri::command]
fn show(state: tauri::State<Mutex<AppState>>, app: AppHandle) {
    let state = state.lock().unwrap();
    state.sender.send(Message::SaveTopmost).unwrap();

    std::thread::sleep(Duration::from_millis(100)); //wait to get topmost to finish
    state.sender.send(Message::RequestHints).unwrap();

    app.emit_all("show", ()).unwrap();
    let window = app.get_window("main").unwrap();
    set_output_size(&window);
    show_window(app.clone());

    window.set_focus().unwrap();
}

///full screen for hints
fn set_full_size(window: &Window) {
    eprintln!("setting full size");
    let monitor = window.current_monitor().unwrap().unwrap();
    let size = monitor.size();
    window.hide().unwrap();
    window
        .set_size(Size::Physical(PhysicalSize {
            width: size.width,
            height: size.height,
        }))
        .unwrap();
    window
        .set_position(Position::Physical(PhysicalPosition { x: 0, y: 0 }))
        .unwrap();
    window.show().unwrap();
}

///size for only output
fn set_output_size(window: &Window) {
    let monitor = window.current_monitor().unwrap().unwrap();
    let size = monitor.size();
    let wid = 900;
    let hei = 300;

    window
        .set_size(Size::Physical(PhysicalSize {
            width: wid,
            height: hei,
        }))
        .unwrap();
    window
        .set_position(Position::Physical(PhysicalPosition {
            x: (size.width - wid) as i32 / 2,
            y: (size.height - hei) as i32 / 2,
        }))
        .unwrap();
}

enum Message {
    AppHandle(AppHandle),
    UpdateInput(String),
    RequestHints,
    Invoke(String, Action),
    SaveTopmost,
}
fn create_hints(elements: &[UiElement]) -> Vec<Hint> {
    let mut index = HashSet::new();
    let hints: Vec<Hint> = elements
        .iter()
        //.take(600) //hard limit (which is less than 26*26 or 2 chars)
        .map(|e| e.into())
        .map(|e: Hint| {
            let mut e = e;
            let chars = e.text.chars().filter(|a| a.is_alphabetic()).to_vec();
            let one = chars.iter().take(1).collect::<String>().to_uppercase();
            let two = chars.iter().take(2).collect::<String>().to_uppercase();

            if !one.is_empty() && !index.contains(&one) {
                e.hint = one.clone();
                index.insert(one);
                return e;
            }
            //go through each of the letters, and return the first one that isn't in the index
            for c in 'A'..='Z' {
                if !index.contains(&c.to_string()) {
                    e.hint = c.to_string();
                    index.insert(c.to_string());
                    return e;
                }
            }
            //else try 2
            if !two.is_empty() && !index.contains(&two) {
                e.hint = two.clone();
                index.insert(two);
                return e;
            }
            //go through every combination of 2 letters
            for c1 in 'A'..='Z' {
                for c2 in 'A'..='Z' {
                    let s = format!("{}{}", c1, c2);
                    if !index.contains(&s) {
                        e.hint = s.clone();
                        index.insert(s);
                        return e;
                    }
                }
            }
            //go through every combination of 3 letters
            for c1 in 'A'..='Z' {
                for c2 in 'A'..='Z' {
                    for c3 in 'A'..='Z' {
                        let s = format!("{}{}{}", c1, c2, c3);
                        if !index.contains(&s) {
                            e.hint = s.clone();
                            index.insert(s);
                            return e;
                        }
                    }
                }
            }

            //should not arrive here
            unreachable!("should be less than 26*26 elements");
        })
        .collect();
    hints
}
fn worker(rec: Receiver<Message>, debug: bool, show_taskbar: bool) {
    //windows::get_elements_mozilla();

    let mut app = None;
    let mut auto = get_accessibility(debug, show_taskbar);
    auto.has_permissions();
    let mut hints: Vec<Hint> = vec![];
    let mut elements: Vec<UiElement> = vec![];
    loop {
        if let Ok(msg) = rec.recv() {
            // windows::get_elements_mozilla();

            match msg {
                Message::AppHandle(ah) => app = Some(ah),
                Message::UpdateInput(inp) => {
                    let app = app.as_ref().unwrap();
                    let matches = do_matching(&hints, inp);
                    println!("matches: {}", matches.len());
                    app.emit_all("update_results", matches).unwrap();
                }
                Message::RequestHints => {
                    elements = auto.get_elements();
                    hints = create_hints(&elements);
                    let app = app.as_ref().unwrap();
                    app.trigger_global("go_full", None);

                    //std::thread::sleep(Duration::from_millis(100)); //wait for results
                    app.emit_all("update_results", hints.iter().collect::<Vec<&Hint>>())
                        .unwrap();
                }
                Message::Invoke(hid, action) => {
                    println!("searching for {}", hid);
                    if let Some(hindex) = hints.iter().position(|h| h.hint == hid) {
                        let ele = &elements[hindex];
                        auto.invoke(ele, action);
                    } else {
                        println!(
                            "no hint found for {} in {:?}",
                            hid,
                            hints.iter().map(|a| a.hint.clone()).collect::<Vec<_>>()
                        );
                    }
                }
                Message::SaveTopmost => {
                    auto.save_topmost();
                }
            }
        }
    }
}

fn get_accessibility(debug: bool, show_taskbar: bool) -> impl AccessibilityCalls {
    #[cfg(target_os = "macos")]
    return mac::Osx::new();
    #[cfg(target_os = "windows")]
    return windows::Windows::new(debug, show_taskbar);
}

///exact hints first, then fuzzy
fn do_matching(hints: &[Hint], inp: String) -> Vec<&Hint> {
    if inp.is_empty() {
        return hints.iter().to_vec();
    }
    let matcher = SkimMatcherV2::default();
    //get 1 or 0 exact matches
    let exact_hints = hints
        .iter()
        .filter(|h| h.hint == inp.to_uppercase())
        .map(|a| &a.hint)
        .to_hashset();
    let exact = hints
        .iter()
        .filter(|h| exact_hints.contains(&h.hint))
        .to_vec();

    let mut matches = hints
        .iter()
        .filter(|a| !exact_hints.contains(&a.hint))
        .map(|h| (h, matcher.fuzzy_match(&h.text, &inp).unwrap_or_default()))
        .filter(|(_, score)| *score > 0)
        .to_vec();
    matches.sort_by(|a, b| b.1.cmp(&a.1));
    let sorted = matches.iter().filter(|a| a.1 > 0).map(|a| a.0).to_vec();
    //matching
    // println!(
    //     "exact: {:?} then {} others. min {} med{} max{}",
    //     exact,
    //     sorted.len(),
    //     matches[0].1,
    //     matches[matches.len() / 2].1,
    //     matches[matches.len() - 1].1
    // );
    exact.into_iter().chain(sorted).collect()
}

fn show_window(app: AppHandle) {
    #[cfg(target_os = "macos")]
    app.show().unwrap();
    #[cfg(not(target_os = "macos"))]
    app.get_window("main").unwrap().show().unwrap();
}
fn hide_window(app: AppHandle) {
    #[cfg(target_os = "macos")]
    app.hide().unwrap();
    #[cfg(not(target_os = "macos"))]
    app.get_window("main").unwrap().hide().unwrap();
}
#[derive(Debug, Serialize, Deserialize)]
struct Hint {
    text: String,
    hint: String,
    x: i32,
    y: i32,
    x_offset: i32,
    y_offset: i32,
    width: i32,
    height: i32,
    control: String,
    parent: String,
}

impl From<&UiElement> for Hint {
    fn from(e: &UiElement) -> Self {
        Hint {
            text: e.name.to_string(),
            hint: String::new(),
            x: e.x,
            y: e.y,
            x_offset: e.x_offset,
            y_offset: e.y_offset,
            width: e.width,
            height: e.height,
            control: e.control.clone(),
            parent: e.parent.clone(),
        }
    }
}
