use std::{cell::Cell, fmt::Display, sync::atomic::AtomicBool};

use accessibility::{
    AXAttribute, AXUIElement, AXUIElementAttributes, TreeVisitor, TreeWalkerFlow,
};
use active_win_pos_rs::get_active_window;
use core_foundation::{
    base::CFType,
    string::CFString,
};
use sysinfo::{PidExt, ProcessExt, ProcessRefreshKind, RefreshKind, SystemExt};

use crate::traits::{AccessibilityCalls, Action, UiElement};

pub struct Osx {
    topmost_pid: Option<i32>,
    dock_pid: Option<i32>,
    child_pids: Vec<i32>,
}

impl Osx {
    pub fn new() -> Self {
        Self {
            topmost_pid: None,
            dock_pid: None,
            child_pids: vec![],
        }
    }
}
impl AccessibilityCalls for Osx {
    fn get_elements(&self) -> Vec<UiElement> {
        let mut elements = vec![];

        //first get dock
        if let Some(pid) = self.dock_pid {
            let dockvisitor = DockVisitor::new();

            let els = accessibility::ui_element::AXUIElement::application(pid);
            let walker = accessibility::TreeWalker::new();

            walker.walk(&els, &dockvisitor);
            elements.extend(dockvisitor.elements.take());
            //SHIFT TO RIGHT OF DOCK
            elements.iter_mut().for_each(|e: &mut UiElement| {
                e.x_offset = 50;
            });
        }

        if self.topmost_pid.is_none() {
            return elements;
        }

        let visitor = MyVisitor::new();
        //let root = accessibility::ui_element::AXUIElement::system_wide();
        let mut pids = vec![self.topmost_pid.unwrap_or_default()];
        pids.extend(self.child_pids.to_vec());
        for pid in pids {
            let els = accessibility::ui_element::AXUIElement::application(pid);
            let walker = accessibility::TreeWalker::new();

            walker.walk(&els, &visitor);
        }
        elements.extend(visitor.elements.take());

        elements
    }

    fn invoke(&self, element: &UiElement, action: Action) {
        let element1 = element.clone();
        //check if in dock
        let els = accessibility::ui_element::AXUIElement::application(self.dock_pid.unwrap());
        if let Some(ele) = accessibility::ElementFinder::new(
            move |f| {
                let a: UiElement = f.into();
                a.name == element1.name
                    && a.class == element1.class
                    && a.x == element1.x
                    && a.y == element1.y
            },
            None,
        )
        .find(&els)
        {
            let _ = match action {
                Action::LeftClick => ele.perform_action(&CFString::new("AXPress")),
                Action::RightClick => ele.perform_action(&CFString::new("AXShowMenu")),
            };
        }
        //else in window

        let els = accessibility::ui_element::AXUIElement::application(self.topmost_pid.unwrap());
        let element = element.clone();
        if let Some(ele) = accessibility::ElementFinder::new(
            move |f| {
                let a: UiElement = f.into();
                a.name == element.name
                    && a.class == element.class
                    && a.x == element.x
                    && a.y == element.y
            },
            None,
        )
        .find(&els)
        {
            let _ = match action {
                Action::LeftClick => ele.perform_action(&CFString::new("AXPress")),
                Action::RightClick => ele.perform_action(&CFString::new("AXShowMenu")),
            };
        }
    }

    fn save_topmost(&mut self) {
        let sys = sysinfo::System::new_with_specifics(
            RefreshKind::new().with_processes(ProcessRefreshKind::new().with_user()),
        );
        let procs = sys.processes();

        //dock
        if let Some(dock) = procs.values().find(|a| a.name() == "Dock") {
            self.dock_pid = Some(dock.pid().as_u32() as i32);
        }

        let win = get_active_window();
        if let Ok(win) = win {
            self.topmost_pid = Some(win.process_id as i32);

            
        }

    }

    fn has_permissions(&self) -> bool {
        macos_accessibility_client::accessibility::application_is_trusted_with_prompt()
    }
}

struct MyVisitor {
    level: Cell<usize>,
    elements: Cell<Vec<UiElement>>,
    found_window: AtomicBool, //we only want the first window (topmost)
}

impl MyVisitor {
    pub fn new() -> Self {
        Self {
            level: Cell::new(0),
            elements: Cell::new(vec![]),
            found_window: AtomicBool::new(false),
        }
    }
}

impl TreeVisitor for MyVisitor {
    fn enter_element(&self, element: &AXUIElement) -> TreeWalkerFlow {
        self.level.replace(self.level.get() + 1);

       /* let role = get_control(element);
        let title = get_name(element);
        let indent = " ".repeat(self.level.get());
        let pos = get_pos(element);
        if title.contains("main.rs")
            || AXUIElementDisplay(element.clone())
                .to_string()
                .contains("main.rs")
        {
            println!["{indent} {:?}- {:?} {pos:?} ", title, role];
        }*/
        if must_include(element) {
            let uie: UiElement = element.into();
            let mut old = self.elements.take();
            old.push(uie);
            self.elements.set(old);
            
            if !must_descend(element) {
                return TreeWalkerFlow::SkipSubtree;
            } else {
                // let displ = AXUIElementDisplay(element.clone()).to_string();
                // if displ.contains("main.rs") {
                //     println!("Not descending into {displ}",);
                // }
            }
        } else {
            // let displ = AXUIElementDisplay(element.clone()).to_string();
            // if displ.contains("main.rs") {
            //     println!("not including {displ}",);
            // }
        }

        if get_control(element) == "AXWindow" {
            let found = self.found_window.load(std::sync::atomic::Ordering::Relaxed);
            if found {
                return TreeWalkerFlow::SkipSubtree;
            } else {
                self.found_window
                    .swap(true, std::sync::atomic::Ordering::Relaxed);
            }
        }

        TreeWalkerFlow::Continue
    }

    fn exit_element(&self, _element: &AXUIElement) {
        self.level.replace(self.level.get() - 1);
    }
}

impl From<&AXUIElement> for UiElement {
    fn from(element: &AXUIElement) -> Self {

        let name = get_name(element);
        let control = get_control(element);
        let pos = get_pos(element);
        let size = get_size(element);

        UiElement {
            id: "".to_string(),
            name,
            x: pos.0,
            y: pos.1,
            x_offset: 0,
            y_offset: 0,
            width: size.0,
            height: size.1,
            control,
            item: "".to_string(), //value                .label_value()                .unwrap_or(CFString::from(""))                .to_string(),
            class: "".to_string(),
            pid: 0,
            parent: "".to_string(),
        }
    }
}

fn get_control(element: &AXUIElement) -> String {
    element
        .role()
        .unwrap_or(CFString::from("Unknown"))
        .to_string()
}

fn must_include(element: &AXUIElement) -> bool {
    //return true;
    let mut incl = true;
    //only if we can left or right click
    if let Ok(acts) = element.action_names() {
        incl &= acts
            .iter()
            .map(|a| a.to_string())
            .filter(|a| a == "AXPress" || a == "AXShowMenu")
            .count()
            > 0;
    }
    //return incl;

    incl &= get_name(element).len() > 0; //only include if has text

    //return incl;

    incl &= match element
        .role()
        .unwrap_or(CFString::from("Unknown"))
        .to_string()
        .as_str()
    {
         "AXStaticText" | "AXMenu" | "AXToolbar" | "AXApplication" | "AXWebArea"
        | "AXTabGroup" | "AXOutline" | "AXHeading" => false,

        "AXRow" | "AXGroup" //vscode uses these two for filelist
        
        | "AXButton" | "AXCheckBox" | "AXMenuItem" | "AXRadioButton"
        | "AXPopUpButton" => true,
        _ => true,
    };
    incl
}

fn must_descend(element: &AXUIElement) -> bool {
    let mut must = true;
    let vis = if let Ok(vis) = element.visible_children() {
        vis.len()
    } else {
        0
    };
    must &= vis > 0;

    must
}

fn get_name(element: &AXUIElement) -> String {
    let title = element.title().unwrap_or(CFString::from("")).to_string();
    let desc = element
        .description()
        .unwrap_or(CFString::from(""))
        .to_string();
    let text = if desc.is_empty() { title } else { desc };
    text
}

fn get_pos(element: &AXUIElement) -> (i32, i32) {
    let mut pos = (0, 0);
    if let Ok(pos2) = element.attribute(&AXAttribute::new(&CFString::new("AXPosition"))) {
        pos = to_pos(pos2);
    }
    pos
}

fn get_size(element: &AXUIElement)->(i32,i32){
    let mut size = (0, 0);
    if let Ok(size2) = element.attribute(&AXAttribute::new(&CFString::new("AXSize"))) {
        size = to_size(size2);
    }
    size
}

///HACK. TODO: find out how to convert CFType to pos
fn to_pos(cfstr: CFType) -> (i32, i32) {
    let a = format!("{cfstr:?}");
    //find x, take to .
    //find y, take to .
    let xind = a.find("x:").unwrap();
    let xafter = &a[xind + 2..];
    let xend = xafter.find(".").unwrap();
    let x = xafter[..xend].to_string().parse().unwrap();
    let yind = xafter.find("y:").unwrap();
    let yafter = &xafter[yind + 2..];
    let yend = yafter.find(".").unwrap();
    let y = yafter[..yend].to_string().parse().unwrap();

    (x, y)
}

///HACK. TODO: find out how to convert CFType to size
fn to_size(cfstr: CFType) -> (i32, i32) {
    let a = format!("{cfstr:?}");
    //find x, take to .
    //find y, take to .
    let xind = a.find("w:").unwrap();
    let xafter = &a[xind + 2..];
    let xend = xafter.find(".").unwrap();
    let x = xafter[..xend].to_string().parse().unwrap();
    let yind = xafter.find("h:").unwrap();
    let yafter = &xafter[yind + 2..];
    let yend = yafter.find(".").unwrap();
    let y = yafter[..yend].to_string().parse().unwrap();

    (x, y)
}

struct AXUIElementDisplay(AXUIElement);

impl Display for AXUIElementDisplay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let chil = if let Ok(ch) = self.0.children() {
            ch.len()
        } else {
            0
        };
        writeln!(
            f,
            "{} {} children - {}",
            get_name(&self.0),
            chil,
            get_control(&self.0)
        )?;
        if let Ok(names) = self.0.attribute_names() {
            for name in names.into_iter() {
                /*if &*name == self.0.children.as_CFString() {
                    continue;
                }*/

                if let Ok(value) = self.0.attribute(&AXAttribute::new(&*name)) {
                    writeln!(f, "  |. {}: {:?}", *name, value)?;
                }
            }
        }
        Ok(())
    }
}

struct DockVisitor {
    elements: Cell<Vec<UiElement>>,
}

impl DockVisitor {
    pub fn new() -> Self {
        Self {
            elements: Cell::new(vec![]),
        }
    }
}

impl TreeVisitor for DockVisitor {
    fn enter_element(&self, element: &AXUIElement) -> TreeWalkerFlow {
        let uie: UiElement = element.into();
        //println!("{uie:?}");
        let mut old = self.elements.take();
        old.push(uie);
        self.elements.set(old);

        TreeWalkerFlow::Continue
    }

    fn exit_element(&self, _element: &AXUIElement) {}
}
