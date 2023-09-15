use std::sync::{atomic::AtomicU32, Arc, Mutex};

use crate::traits::{AccessibilityCalls, Action, UiElement};
use active_win_pos_rs::get_active_window;
use uiautomation::{controls::ControlType, Error, UIAutomation, UIElement, UITreeWalker};

pub struct Windows {
    auto: UIAutomation,
    topmost_pid: Option<i32>,
}

impl Windows {
    pub fn new() -> Self {
        Windows {
            auto: uiautomation::UIAutomation::new().unwrap(),
            topmost_pid: None,
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
        let walker = self.auto.get_control_view_walker().unwrap();
        let pid = self.topmost_pid;
        let root_window = self
            .auto
            .create_matcher()
            .depth(2)
            .filter_fn(Box::new(move |e: &UIElement| {
                Ok(e.get_process_id().unwrap() == pid.unwrap())
            }))
            .find_first()
            .unwrap_or(self.auto.get_root_element().unwrap());

        walk(
            &walker,
            &root_window,
            vec.clone(),
            counter.clone(),
            root_window.get_name().unwrap_or_default(),
            self.topmost_pid,
            usize::MAX,
            0,
            |a| must_include(a).unwrap_or_default(),
            |a| must_descend(a).unwrap_or_default(),
        )
        .unwrap();
        /* print(
            &walker,
            &root_window,
            usize::MAX,
            0,
            |a| must_include(a).unwrap_or_default(),
            |a| must_descend(a).unwrap_or_default(),
        )
        .unwrap();*/

        //get from taskbar
        let taskbar = self
            .auto
            .create_matcher()
            .depth(2)
            .classname("Shell_TrayWnd")
            .find_first()
            .unwrap();

        let walker = self.auto.get_control_view_walker().unwrap();

        walk(
            &walker,
            &taskbar,
            vec.clone(),
            counter.clone(),
            "taskbar".into(),
            None,
            2,
            0,
            |_a| true,
            |a| must_descend(a).unwrap_or_default(),
        )
        .unwrap();

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
        let ele;
        //find in taskbar
        let taskbar = self
            .auto
            .create_matcher()
            .depth(2)
            .classname("Shell_TrayWnd")
            .find_first()
            .unwrap();

        let cond = self
            .auto
            .create_property_condition(
                uiautomation::types::UIProperty::ClassName,
                element.class.clone().into(),
                None,
            )
            .and(self.auto.create_property_condition(
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
            let win = self
                .auto
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
        /*
        self.topmost_pid = None;
        if let Ok(foc) = self
            .auto
            .create_matcher()
            .filter_fn(Box::new(|a: &UIElement| a.has_keyboard_focus()))
            .find_first()
        {
            if let Ok(_) = foc.get_native_window_handle() {
                if let Ok(pi) = foc.get_process_id() {
                    self.topmost_pid = Some(pi)
                }
            }
        }*/
        let win = get_active_window();
        if let Ok(win) = win {
            self.topmost_pid = Some(win.process_id as i32);
        }
        println!("active pid: {:?}", self.topmost_pid);
    }

    fn has_permissions(&self) -> bool {
        true
    }
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
        let rect = element.get_bounding_rectangle().unwrap();
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
    include_fn: fn(&UIElement) -> bool,
    descend_fn: fn(&UIElement) -> bool,
) -> Result<()> {
    let start = std::time::Instant::now();
    counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if level > max {
        return Ok(());
    }
    //dont check wrong pid
    if pid.is_some() && Some(element.get_process_id()?) != pid {
        return Ok(());
    }

    if include_fn(element) {
        let mut el: UiElement = element.into();
        el.parent = parent.clone();
        vec.lock().unwrap().push(el);
    }
    if !descend_fn(element) {
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
            descend_fn,
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
                descend_fn,
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
/*
struct UI2(UIElement);
impl Display for UI2 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let rect = self.0.get_bounding_rectangle().unwrap();
        let x = rect.get_left();
        let y = rect.get_top();
        let width = rect.get_width();
        let height = rect.get_height();
        let typ = format!("{:?}", self.0.get_control_type().unwrap());
        let classname = self.0.get_classname().unwrap();
        let it = self.0.get_item_type().unwrap();
        let name = self.0.get_name().unwrap();
        let pid = self.0.get_process_id().unwrap();
        write!(
            f,
            "UIElement {{  x: {}, y: {}, width: {}, height: {}, typ: {}, classname: {}  {it} {name} {pid}}}",
           x, y, width, height, typ, classname
        )
    }
}*/

fn must_include(element: &UIElement) -> Result<bool> {
    // return Ok(true);
    if element.is_offscreen()? {
        //  println!("Excluding offscreen element {:?}", element);
        return Ok(false);
    }

    //return Ok(true);

    Ok(match element.get_control_type()? {
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
    })
}

fn must_descend(element: &UIElement) -> Result<bool> {
    Ok(match element.get_control_type()? {
        ControlType::DataGrid => false, //or get lots in excel
        _ => true,
    })
}
