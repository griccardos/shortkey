use std::{
    fmt::Display,
    sync::{atomic::AtomicU32, Arc, Mutex},
    vec,
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
}

impl Windows {
    pub fn new(debug: bool, show_taskbar: bool) -> Self {
        //test();
        Windows {
            topmost_pid: None,
            debug,
            show_taskbar,
            topmost_children: Vec::new(),
        }
    }
}

impl AccessibilityCalls for Windows {
    fn get_elements(&self) -> Vec<UiElement> {
        let start = std::time::Instant::now();
        println!("Starting to get elements");
        let counter = Arc::new(AtomicU32::new(0));
        let vec: Arc<Mutex<Vec<UiElement>>> = Arc::new(Mutex::new(Vec::new()));
        //get from upmost window
        let pid = self.topmost_pid;
        get_elements_pid(vec.clone(), counter.clone(), pid, self.debug);
        //let els = get_elements_mozilla();
        //counter.fetch_add(els.len() as u32, std::sync::atomic::Ordering::Relaxed);
        //vec.lock().unwrap().extend(els);

        //get from taskbar
        if self.show_taskbar {
            get_elements_taskbar(vec.clone(), counter.clone());
        }

        let vec = vec.lock().unwrap();
        let vec: Vec<UiElement> = vec
            .iter()
            .filter(|a| a.name != "") //exclude empty names
            .map(|a| a.clone())
            .collect();
        /*
                let vec: Vec<UiElement> = self
                    .auto
                    .create_matcher()
                    /* .filter_fn(Box::new(|f: &UIElement| {
                        let include = f.get_control_type()? == ControlType::TabItem
                            || f.get_control_type()? == ControlType::Button;
                        let required = f.is_enabled().unwrap()
                            && f.is_control_element().unwrap()
                            && !f.is_offscreen().unwrap();
                        Ok(include && required)
                    }))*/
                    .filter_fn(Box::new({
                        let pid = self.topmost_pid.clone();
                        move |a: &UIElement| must_include(a, pid)
                    }))
                    .find_all()
                    .unwrap_or_default()
                    .iter()
                    .map(|a| a.into())
                    .collect();
        */
        println!(
            "Got {} elements out of {} in {}ms",
            vec.len(),
            counter.load(std::sync::atomic::Ordering::Relaxed),
            start.elapsed().as_millis()
        );

        vec
    }

    fn invoke(&self, element: &UiElement, action: Action) {
        //it will either be in start button or active window
        let start = std::time::Instant::now();
        let auto = UIAutomation::new().unwrap();

        let ele;
        //find in taskbar
        let taskbar = auto
            .create_matcher()
            .depth(2)
            .classname("Shell_TrayWnd")
            .find_first()
            .unwrap();

        let cond = auto
            .create_property_condition(
                uiautomation::types::UIProperty::ClassName,
                element.class.clone().into(),
                None,
            )
            .and(auto.create_property_condition(
                uiautomation::types::UIProperty::Name,
                element.name.clone().into(),
                None,
            ))
            .unwrap();
        let taskbar_item = taskbar.find_first(uiautomation::types::TreeScope::Subtree, &cond);
        //if it is a taskbar item and same pid as taskbar
        if taskbar_item.is_ok()
            && taskbar_item.as_ref().unwrap().get_process_id() == taskbar.get_process_id()
        {
            ele = taskbar_item
        } else {
            let win = auto
                .create_matcher()
                .match_name(element.name.clone())
                .classname(element.class.clone())
                .filter_fn(Box::new({
                    let (x, y) = (element.x, element.y);
                    move |e: &UIElement| {
                        e.get_bounding_rectangle()
                            .map(|r| r.get_left() == x && r.get_top() == y)
                    }
                }))
                .find_first();
            ele = win;
        }
        if let Ok(ele) = ele {
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
pub fn get_elements_mozilla() -> Vec<UiElement> {
    let auto = UIAutomation::new().unwrap();

    let root_window = auto
        .create_matcher()
        .from(auto.get_root_element().unwrap())
        .timeout(10000)
        .classname("MozillaWindowClass")
        .find_first()
        .unwrap();
    let m2 = auto
        .create_matcher()
        .timeout(10000)
        .from(root_window)
        // .filter_fn(Box::new(|f: &UIElement| {
        //     let rec = f.get_bounding_rectangle().unwrap();
        //     if rec.get_left() > 0 && rec.get_top() > 0 {
        //         return Ok(true);
        //     } else {
        //         return Ok(false);
        //     }
        // }))
        .find_all()
        .unwrap_or(vec![])
        .iter()
        .map(|a| UiElement::from(a))
        .to_vec();
    println!(
        "olar {:?} ",
        m2.iter().filter(|a| a.name.contains("olar")).to_vec()
    );
    m2
}
fn get_elements_pid(
    vec: Arc<Mutex<Vec<UiElement>>>,
    counter: Arc<AtomicU32>,
    pid: Option<i32>,
    debug: bool,
) {
    let auto = UIAutomation::new().unwrap();

    let root_window = auto
        //.get_focused_element()
        .create_matcher()
        .depth(5)
        .filter_fn(Box::new(move |e: &UIElement| {
            Ok(e.get_process_id().unwrap() == pid.unwrap())
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
        |a| must_include(a).unwrap_or_default(),
        debug,
    )
    .unwrap();
}
fn get_elements_taskbar(vec: Arc<Mutex<Vec<UiElement>>>, counter: Arc<AtomicU32>) {
    let auto = UIAutomation::new().unwrap();

    let taskbar = auto
        .create_matcher()
        .depth(2)
        .classname("Shell_TrayWnd")
        .find_first()
        .unwrap();

    let walker = auto.get_control_view_walker().unwrap();

    walk(
        &walker,
        &taskbar,
        vec.clone(),
        counter.clone(),
        "taskbar".into(),
        None,
        2,
        0,
        |_a| (true, "".into()),
        false,
    )
    .unwrap();
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
    vec: Arc<Mutex<Vec<UiElement>>>,
    counter: Arc<AtomicU32>,
    parent: String,
    pid: Option<i32>,
    max: usize,
    level: usize,
    include_fn: fn(&UIElement) -> (bool, String),
    debug: bool,
) -> Result<()> {
    counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if level > max {
        return Ok(());
    }
    let mut correct_pid = true;
    //dont check wrong pid
    let el_pid = element.get_process_id()?;
    if pid.is_some() && Some(el_pid) != pid {
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

    let incl = include_fn(element);
    if incl.0 && correct_pid {
        let mut el: UiElement = element.into();
        el.parent = parent.clone();
        vec.lock().unwrap().push(el);
    } else {
        if debug {
            let mut el: UiElement = element.into();
            el.name += &format!(" (EXCLUDING {})", incl.1);

            //println!("excluding {:?} [{}]", el, UI2(element.clone()));

            vec.lock().unwrap().push(el);
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
            include_fn,
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
                include_fn,
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

fn print(
    walker: &UITreeWalker,
    element: &UIElement,
    max: usize,
    level: usize,
    include_fn: fn(&UIElement) -> bool,
    descend_fn: fn(&UIElement) -> bool,
) -> Result<()> {
    let start = std::time::Instant::now();
    if level > max {
        return Ok(());
    }

    if !descend_fn(element) {
        return Ok(());
    }

    if let Ok(child) = walker.get_first_child(&element) {
        print(walker, &child, max, level + 1, include_fn, descend_fn)?;

        let mut next = child;
        while let Ok(sibling) = walker.get_next_sibling(&next) {
            print(walker, &sibling, max, level + 1, include_fn, descend_fn)?;

            next = sibling;
        }
    }
    if include_fn(element) {
        println!(
            "PRINT:{},{},{:?},\"{}\"",
            start.elapsed().as_millis(),
            level,
            element.get_control_type().unwrap(),
            element.get_name().unwrap_or_default(),
        );
    }
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
