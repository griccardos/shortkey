use std::fmt::Display;

use crate::traits::{AccessibilityCalls, Action, UiElement};
use active_win_pos_rs::get_active_window;
use easier::prelude::ToCollectionIteratorExtension;
use uiautomation::{controls::ControlType, Error, UIAutomation, UIElement};

pub struct Windows {
    topmost: Option<Parent>,
    debug: bool,
    show_taskbar: bool,
    elements: Vec<UIElement>,
}

#[derive(Debug)]
struct Parent {
    pid: i32,
    name: String,
}

impl Windows {
    pub fn new(debug: bool, show_taskbar: bool) -> Self {
        Windows {
            topmost: None,
            debug,
            show_taskbar,
            elements: Vec::new(),
        }
    }
}

impl AccessibilityCalls for Windows {
    fn get_elements(&mut self) -> Vec<UiElement> {
        let start = std::time::Instant::now();
        println!("Starting to get elements");
        self.elements.clear();
        let mut result = vec![];
        //get from upmost window
        if let Some(topmost) = self.topmost.as_ref() {
            let elements = get_elements_pid(topmost.pid, self.debug);
            result.extend(elements.iter().map(|a| {
                let mut el = UiElement::from(a);
                el.parent = topmost.name.clone();
                el
            }));
            println!(
                "got {} topmost elements in {}ms",
                elements.len(),
                start.elapsed().as_millis()
            );
        }

        //get from taskbar
        if self.show_taskbar {
            let taskbarelements = get_elements_taskbar();
            result.extend(taskbarelements.iter().map(|a| {
                let mut el = UiElement::from(a);
                el.parent = "taskbar".into();
                el
            }));

            println!(
                "got {} taskbar elements in {}ms",
                taskbarelements.len(),
                start.elapsed().as_millis()
            );
        }

        let uivec = result
            .into_iter()
            .filter(|a| !a.name.is_empty()) //exclude empty names
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
            .find(|&a| &UiElement::from(a) == element);
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
            self.topmost = Some(Parent {
                pid: win.process_id as i32,
                name: win.app_name,
            });
            println!("active window: {:?} ", self.topmost);
        } else {
            println!("no active window");
        }
    }

    fn has_permissions(&self) -> bool {
        true
    }
}

fn invoke_element(ele: &UIElement, action: Action) {
    println!("invoking {}", UI2(ele.clone()));
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

fn get_elements_from_root(root_window: &UIElement, debug: bool) -> Vec<UIElement> {
    let auto = UIAutomation::new().unwrap();

    /*

      let vec: Arc<Mutex<Vec<UIElement>>> = Arc::new(Mutex::new(Vec::new()));
     let counter = Arc::new(AtomicU32::new(0));
    let walker = auto.get_control_view_walker().unwrap();
     let start = std::time::Instant::now();
      println!(
         "find_all {} in {}ms",
         els.unwrap().len(),
         start.elapsed().as_millis()
     );

     walk(
         &walker,
         &root_window,
         vec.clone(),
         counter.clone(),
         pid,
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
     vec2.clone()*/
    let els = auto.create_matcher().from(root_window.clone()).find_all();
    els.unwrap_or_default()
        .into_iter()
        .filter(|a| {
            let incl = must_include(&a);
            if let Ok(incl) = incl {
                incl.0
            } else if debug {
                println!("excluding {} because {:?}", UI2(a.clone()), incl);
                false
            } else {
                false
            }
        })
        .collect()
}
fn get_elements_pid(pid: i32, debug: bool) -> Vec<UIElement> {
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
        println!("no topmost window found");
        auto.get_root_element().unwrap()
    };

    get_elements_from_root(&root_window, debug)
}
fn get_elements_taskbar() -> Vec<UIElement> {
    let auto = UIAutomation::new().unwrap();
    let root_window = auto
        .create_matcher()
        .depth(2)
        .classname("Shell_TrayWnd")
        .find_first()
        .unwrap();

    get_elements_from_root(&root_window, false)
}

impl PartialEq for &UiElement {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.control == other.control
            && self.x == other.x
            && self.y == other.y
            && self.width == other.width
            && self.height == other.height
    }
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
        let pid = element.get_process_id().unwrap();
        // println!("{}:{}:{:?}", name, id, element);
        UiElement {
            name,
            x,
            y,
            width,
            height,
            control,
            pid,
            x_offset: 0,
            y_offset: 0,
            parent: String::new(),
        }
    }
}

pub type Result<T> = core::result::Result<T, Error>;
/*fn walk(
    walker: &UITreeWalker,
    element: &UIElement,
    vec: Arc<Mutex<Vec<UIElement>>>,
    counter: Arc<AtomicU32>,
    pid: i32,
    level: usize,
    debug: bool,
) -> Result<()> {
    counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if level > 20 {
        return Ok(());
    }
    let mut correct_pid = true;
    //dont check wrong pid
    let el_pid = element.get_process_id()?;
    if el_pid != pid {
        correct_pid = false;
        // return Ok(());
    }

    let incl = must_include(element)?;
    if incl.0 && correct_pid {
        vec.lock().unwrap().push(element.clone());
    } else if debug {
        if !correct_pid {
            println!("excluding {} because wrong pid", UI2(element.clone()));
        } else {
            println!("excluding {} because {}", UI2(element.clone()), incl.1);
        }
        vec.lock().unwrap().push(element.clone());
    }

    if !must_descend(element)? && !debug {
        return Ok(());
    }

    if let Ok(child) = walker.get_first_child(element) {
        walk(
            walker,
            &child,
            vec.clone(),
            counter.clone(),
            pid,
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
                pid,
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
}*/

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
        Ok((true, String::new()))
    } else {
        Ok((false, "unallowed control".to_string()))
    }
}
/*
fn must_descend(element: &UIElement) -> Result<bool> {
    Ok(match element.get_control_type()? {
        ControlType::DataGrid => false, //or get lots in excel
        _ => true,
    })
}
*/
