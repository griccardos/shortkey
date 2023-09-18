use std::{
    fmt::Display,
    sync::{atomic::AtomicU32, Arc, Mutex},
};

use crate::traits::{AccessibilityCalls, Action, UiElement};
use active_win_pos_rs::get_active_window;
use easier::prelude::ToCollectionIteratorExtension;
use sysinfo::{ProcessExt, SystemExt};
use uiautomation::{controls::ControlType, Error, UIAutomation, UIElement, UITreeWalker};

pub struct Windows {
    topmost_pid: Option<i32>,
    topmost_children: Vec<i32>,
    debug: bool,
    show_taskbar: bool,
    elements: Vec<UIElement>,
}

impl Windows {
    pub fn new(debug: bool, show_taskbar: bool) -> Self {
        Windows {
            topmost_pid: None,
            debug,
            show_taskbar,
            topmost_children: Vec::new(),
            elements: Vec::new(),
        }
    }
}

impl AccessibilityCalls for Windows {
    fn get_elements(&mut self) -> Vec<UiElement> {
        let start = std::time::Instant::now();
        println!("Starting to get elements");
        let mut vec: Vec<UIElement> = Vec::new();
        //get from upmost window
        let pid = self.topmost_pid;
        let elements = get_elements_pid(pid, self.debug);
        vec.extend(elements);

        //get from taskbar
        if self.show_taskbar {
            let elements = get_elements_taskbar();
            vec.extend(elements);
        }

        self.elements = vec.clone();

        let uivec = vec
            .into_iter()
            .map(|a| UiElement::from(&a))
            .filter(|a| a.name != "") //exclude empty names
            .to_vec();

        println!(
            "Got {} elements in {}ms",
            uivec.len(),
            start.elapsed().as_millis()
        );

        uivec
    }

    fn invoke(&self, element: &UiElement, action: Action) {
        //it will either be in start button or active window
        let start = std::time::Instant::now();

        let ele = self
            .elements
            .iter()
            .find(|&a| UiElement::from(a).id == element.id);
        if let Some(ele) = ele {
            invoke_element(ele, action);
        } else {
            println!("no element found for {:?}", element);
        }
        println!("invoked in {}ms", start.elapsed().as_millis());
    }
    fn save_topmost(&mut self) {
        let win = get_active_window();
        if let Ok(win) = win {
            self.topmost_pid = Some(win.process_id as i32);
        }
        println!("active pid: {:?}", self.topmost_pid);

        let sys = sysinfo::System::new_with_specifics(
            sysinfo::RefreshKind::new()
                .with_processes(sysinfo::ProcessRefreshKind::new().with_user()),
        );
        if let Some(pid) = self.topmost_pid {
            self.topmost_children = sys
                .processes()
                .iter()
                .filter(|a| a.1.parent() == Some(sysinfo::Pid::from(pid as usize)))
                .map(|a| usize::from(a.0.clone()) as i32)
                .map(|a| {
                    println!("child pid: {:?}", a);
                    a
                })
                .to_vec();
        }
    }

    fn has_permissions(&self) -> bool {
        true
    }
}

fn invoke_element(ele: &UIElement, action: Action) {
    println!("invoking {:?}", ele.get_name().unwrap());
    let mouse = uiautomation::inputs::Mouse::new().move_time(1);
    //let old = uiautomation::inputs::Mouse::get_cursor_pos().unwrap();
    let rect = ele.get_bounding_rectangle().unwrap();
    let pos = uiautomation::types::Point::new(
        rect.get_left() + rect.get_width() / 2,
        rect.get_top() + rect.get_height() / 2,
    );
    mouse.move_to(pos).unwrap();
    match action {
        Action::LeftClick => {
            ele.click().unwrap();
        }
        Action::RightClick => ele.right_click().unwrap(),
    }
    // mouse.move_to(old).unwrap();
}

fn get_elements_pid(pid: Option<i32>, debug: bool) -> Vec<UIElement> {
    if pid.is_none() {
        return Vec::new();
    }
    let pid = pid.unwrap();
    let vec: Arc<Mutex<Vec<UIElement>>> = Arc::new(Mutex::new(Vec::new()));
    let counter = Arc::new(AtomicU32::new(0));
    let auto = UIAutomation::new().unwrap();

    let root_window = auto
        .create_matcher()
        .depth(5)
        .filter_fn(Box::new(move |e: &UIElement| {
            Ok(e.get_process_id().unwrap() == pid)
        }))
        .find_first();
    let root_window = if let Ok(win) = root_window {
        win
    } else {
        println!("no root window found");
        auto.get_root_element().unwrap()
    };

    println!(
        "root window pid: {:?} {:?} {:?}",
        root_window.get_process_id().unwrap(),
        root_window.get_name().unwrap(),
        root_window.get_classname().unwrap()
    );
    let walker = auto.get_control_view_walker().unwrap();

    walk(
        &walker,
        &root_window,
        vec.clone(),
        counter.clone(),
        root_window.get_name().unwrap_or_default(),
        pid,
        usize::MAX,
        0,
        debug,
    )
    .unwrap();
    println!(
        "pid {pid} elements: {} out of {}",
        vec.lock().unwrap().len(),
        counter.load(std::sync::atomic::Ordering::Relaxed)
    );
    let vec2 = vec.lock().unwrap();
    vec2.clone()
}
fn get_elements_taskbar() -> Vec<UIElement> {
    let vec: Arc<Mutex<Vec<UIElement>>> = Arc::new(Mutex::new(Vec::new()));
    let counter = Arc::new(AtomicU32::new(0));
    let auto = UIAutomation::new().unwrap();

    let taskbar = auto
        .create_matcher()
        .depth(2)
        .classname("Shell_TrayWnd")
        .find_first()
        .unwrap();
    let pid = taskbar.get_process_id().unwrap_or_default();

    let walker = auto.get_control_view_walker().unwrap();

    walk(
        &walker,
        &taskbar,
        vec.clone(),
        counter.clone(),
        "taskbar".into(),
        pid,
        2,
        0,
        false,
    )
    .unwrap();
    println!(
        "taskbar elements: {} out of {}",
        vec.lock().unwrap().len(),
        counter.load(std::sync::atomic::Ordering::Relaxed)
    );
    let vec2 = vec.lock().unwrap();
    vec2.clone()
}

fn getid(ele: &UIElement) -> String {
    format!(
        "{}_{:?}",
        ele.get_name().unwrap(),
        ele.get_control_type().unwrap(),
        //ele.get_process_id().unwrap()
    )
}
impl From<&UIElement> for UiElement {
    fn from(element: &UIElement) -> Self {
        let rect = element.get_bounding_rectangle();
        let rect = if let Ok(re) = rect {
            re
        } else {
            println!("ERROR: no rect");
            uiautomation::types::Rect::default()
        };
        let x = rect.get_left();
        let y = rect.get_top();
        let width = rect.get_width();
        let height = rect.get_height();
        let name = element.get_name().unwrap();
        let control = format!("{:?}", element.get_control_type().unwrap());
        let item = element.get_item_type().unwrap();
        let class = element.get_classname().unwrap();
        let pid = element.get_process_id().unwrap();
        let parent = element.get_classname().unwrap();
        let id = getid(element);
        // println!("{}:{}:{:?}", name, id, element);
        UiElement {
            id,
            name,
            x,
            y,
            width,
            height,
            control,
            item,
            class,
            parent,
            pid,
            x_offset: 0,
            y_offset: 0,
        }
    }
}

pub type Result<T> = core::result::Result<T, Error>;
fn walk(
    walker: &UITreeWalker,
    element: &UIElement,
    vec: Arc<Mutex<Vec<UIElement>>>,
    counter: Arc<AtomicU32>,
    parent: String,
    pid: i32,
    max: usize,
    level: usize,
    debug: bool,
) -> Result<()> {
    counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if level > max {
        return Ok(());
    }
    let mut correct_pid = true;
    //dont check wrong pid
    let el_pid = element.get_process_id()?;
    if el_pid != pid {
        correct_pid = false;
        println!("wrong pid {:?} {:?}", el_pid, pid);
        // return Ok(());
    } else {
        // println!("right pid {:?} {:?}", el_pid, pid);
    }
    // if UiElement::from(element).name.contains("Square root") {
    //     println!(
    //         "including {:?} [{}] pid{:?}",
    //         element,
    //         UI2(element.clone()),
    //         element.get_process_id()
    //     );
    // }

    let incl = must_include(element)?;
    if incl.0 && correct_pid {
        vec.lock().unwrap().push(element.clone());
    } else {
        if debug {
            println!("excluding {} because {}", UI2(element.clone()), incl.1);
            vec.lock().unwrap().push(element.clone());
        }
    }
    if !must_descend(element)? && !debug {
        return Ok(());
    }

    if let Ok(child) = walker.get_first_child(&element) {
        walk(
            walker,
            &child,
            vec.clone(),
            counter.clone(),
            parent.clone(),
            pid,
            max,
            level + 1,
            debug,
        )?;

        let mut next = child;
        while let Ok(sibling) = walker.get_next_sibling(&next) {
            walk(
                walker,
                &sibling,
                vec.clone(),
                counter.clone(),
                parent.clone(),
                pid,
                max,
                level + 1,
                debug,
            )?;

            next = sibling;
        }
    }
    /* println!(
        "{},{},{:?},\"{}\"",
        start.elapsed().as_millis(),
        level,
        element.get_control_type().unwrap(),
        element.get_name().unwrap_or_default(),
    );*/
    Ok(())
}

struct UI2(UIElement);
impl Display for UI2 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let rect = self.0.get_bounding_rectangle().unwrap();

        let x = rect.get_left();
        let y = rect.get_top();
        let width = rect.get_width();
        let height = rect.get_height();
        let typ = format!("{:?}", self.0.get_control_type().unwrap());
        let classname = self.0.get_classname().unwrap_or_default();
        let name = self.0.get_name().unwrap();
        let pid = self.0.get_process_id().unwrap();

        write!(
            f,
            "UIElement {{  x: {}, y: {}, width: {}, height: {}, typ: {}, classname: '{}'   name:'{name}' pid:{pid}}}",
           x, y, width, height, typ, classname
        )
    }
}

fn must_include(element: &UIElement) -> Result<(bool, String)> {
    // return Ok(true);
    if element.is_offscreen()? {
        //  println!("Excluding offscreen element {:?}", element);
        return Ok((false, "Offscreen".into()));
    }

    //return Ok(true);

    let ctype = match element.get_control_type()? {
        ControlType::Button
        | ControlType::ListItem
        | ControlType::TreeItem
        | ControlType::Hyperlink
        | ControlType::ComboBox
        | ControlType::RadioButton
        | ControlType::CheckBox
        | ControlType::TabItem => true,

        ControlType::Text
        | ControlType::ScrollBar
        | ControlType::Menu
        | ControlType::MenuItem
        | ControlType::Edit
        | ControlType::Calendar
        | ControlType::Image
        | ControlType::List
        | ControlType::MenuBar
        | ControlType::ProgressBar
        | ControlType::Slider
        | ControlType::Spinner
        | ControlType::StatusBar
        | ControlType::Tab
        | ControlType::ToolBar
        | ControlType::ToolTip
        | ControlType::Tree
        | ControlType::Custom
        | ControlType::Group
        | ControlType::Thumb
        | ControlType::DataGrid
        | ControlType::DataItem
        | ControlType::Document
        | ControlType::SplitButton
        | ControlType::Window
        | ControlType::Pane
        | ControlType::Header
        | ControlType::HeaderItem
        | ControlType::Table
        | ControlType::TitleBar
        | ControlType::Separator
        | ControlType::SemanticZoom
        | ControlType::AppBar => false,
    };

    if ctype {
        return Ok((true, String::new()));
    } else {
        return Ok((false, "unallowed control".to_string()));
    }
}

fn must_descend(element: &UIElement) -> Result<bool> {
    Ok(match element.get_control_type()? {
        ControlType::DataGrid => false, //or get lots in excel
        _ => true,
    })
}
