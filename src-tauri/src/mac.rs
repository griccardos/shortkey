use std::{cell::Cell, fmt::Display, time::Instant};

use accessibility::{AXAttribute, AXUIElement, AXUIElementAttributes, TreeVisitor, TreeWalkerFlow};
use active_win_pos_rs::get_active_window;
use core_foundation::{base::CFType, string::CFString};

use crate::traits::{AccessibilityCalls, Action, UiElement};

pub struct Osx {
    topmost: Option<Parent>,
    _dock_pid: Option<i32>,
}

#[derive(Default, Debug, Clone)]
struct Parent {
    name: String,
    pid: i32,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

impl Osx {
    pub fn new() -> Self {
        Self {
            topmost: None,
            _dock_pid: None,
        }
    }
}
impl AccessibilityCalls for Osx {
    fn get_elements(&mut self) -> Vec<UiElement> {
        let start = Instant::now();
        let mut elements = vec![];

        //TODO: once we can overlay on the dock, we can add this back
        /*        //first get dock
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
        */
        if self.topmost.is_none() {
            return elements;
        }

        let visitor = MyVisitor::new(self.topmost.clone().unwrap());
        //let root = accessibility::ui_element::AXUIElement::system_wide();
        let pids = vec![self.topmost.as_ref().unwrap().pid];
        for pid in pids {
            let els = accessibility::ui_element::AXUIElement::application(pid);
            let walker = accessibility::TreeWalker::new();

            walker.walk(&els, &visitor);
        }
        elements.extend(visitor.elements.take());
        println!(
            "found {} in {}ms",
            elements.len(),
            start.elapsed().as_millis()
        );

        elements
            .iter_mut()
            .for_each(|a| a.parent = self.topmost.as_ref().unwrap().name.clone());

        elements
    }

    fn invoke(&self, element: &UiElement, action: Action) {
        //check if in dock
        /* let els = accessibility::ui_element::AXUIElement::application(self.dock_pid.unwrap());
        let element1 = element.clone();

        if let Some(ele) = accessibility::ElementFinder::new(
            move |f| {
                let a: UiElement = f.into();
                a.name == element1.name
                    && a.control == element1.control
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
        }*/
        //else in window

        let els =
            accessibility::ui_element::AXUIElement::application(self.topmost.as_ref().unwrap().pid);
        let element = element.clone();
        if let Some(_) = accessibility::ElementFinder::new(
            move |f| {
                let a: UiElement = f.into();
                a.name == element.name
                    && a.control == element.control
                    && a.x == element.x
                    && a.y == element.y
            },
            None,
        )
        .find(&els)
        {
            let x = element.x + element.width / 2;
            let y = element.y + element.height / 2;
            let mouse = mouce::Mouse::new();
            let _ = mouse.move_to(x as usize, y as usize);

            let _ = match action {
                Action::LeftClick => {
                    //ele.perform_action(&CFString::new(left))
                    let _ = mouse.click_button(&mouce::common::MouseButton::Left);
                }
                Action::RightClick => {
                    //ele.perform_action(&CFString::new(right))},
                    let _ = mouse.click_button(&mouce::common::MouseButton::Right);
                }
            };
        } else {
            println!("Could not find element to invoke");
        }
    }

    fn save_topmost(&mut self) {
        /*let sys = sysinfo::System::new_with_specifics(
            RefreshKind::new().with_processes(ProcessRefreshKind::new().with_user()),
        );
        let procs = sys.processes();

        //dock
        if let Some(dock) = procs.values().find(|a| a.name() == "Dock") {
            self.dock_pid = Some(dock.pid().as_u32() as i32);
        }*/

        let win = get_active_window();

        self.topmost = if let Ok(win) = win {
            let par = Parent {
                name: win.app_name,
                pid: win.process_id as i32,
                x: win.position.x as i32,
                y: win.position.y as i32,
                width: win.position.width as i32,
                height: win.position.height as i32,
            };
            println!("Topmost: {par:?}");
            Some(par)
        } else {
            None
        };
    }

    fn has_permissions(&self) -> bool {
        macos_accessibility_client::accessibility::application_is_trusted_with_prompt()
    }
}

struct MyVisitor {
    level: Cell<usize>,
    elements: Cell<Vec<UiElement>>,
    root: Parent, //we only want the first window (topmost)
}

impl MyVisitor {
    pub fn new(root: Parent) -> Self {
        Self {
            level: Cell::new(0),
            elements: Cell::new(vec![]),
            root,
        }
    }
}

impl TreeVisitor for MyVisitor {
    fn enter_element(&self, element: &AXUIElement) -> TreeWalkerFlow {
        //update level
        let new_level = self.level.get() + 1;
        self.level.replace(new_level);

        if let Some(mut uie) = must_include(
            element,
            self.root.x,
            self.root.y,
            self.root.width,
            self.root.height,
        ) {
            //let mut uie: UiElement = element.into();

            //for menu
            if uie.control == "AXMenuBarItem" {
                uie.y_offset = 20;
            }

            let mut old = self.elements.take();
            old.push(uie);
            self.elements.set(old);

            if !must_descend(element) {
                /*println!(
                    "Not descending into {}",
                    AXUIElementDisplay(element.clone())
                );*/
                return TreeWalkerFlow::SkipSubtree;
            }
        } else {
            //let displ = AXUIElementDisplay(element.clone()).to_string();
            // if displ.contains("main.rs") {
            //  println!("not including {displ}",);
            // }
        }

        /* if get_control(element) == "AXWindow" {
            let found = self.found_window.load(std::sync::atomic::Ordering::Relaxed);
            if found {
                return TreeWalkerFlow::SkipSubtree;
            } else {
                self.found_window
                    .swap(true, std::sync::atomic::Ordering::Relaxed);
            }
        }*/

        TreeWalkerFlow::Continue
    }

    fn exit_element(&self, _element: &AXUIElement) {
        self.level.replace(self.level.get() - 1);
    }
}

fn into_element(
    element: &AXUIElement,
    name: String,
    role: String,
    posx: i32,
    posy: i32,
) -> UiElement {
    let size = get_size(element);

    UiElement {
        name,
        x: posx,
        y: posy,
        x_offset: 0,
        y_offset: 0,
        width: size.0,
        height: size.1,
        control: role,
        pid: 0,
        parent: "".to_string(),
    }
}

impl From<&AXUIElement> for UiElement {
    fn from(element: &AXUIElement) -> Self {
        let control = get_role(element);
        let name = get_name(element);
        let pos = get_pos(element);
        let size = get_size(element);

        UiElement {
            name,
            x: pos.0,
            y: pos.1,
            x_offset: 0,
            y_offset: 0,
            width: size.0,
            height: size.1,
            control,
            pid: 0,
            parent: "".to_string(),
        }
    }
}

fn must_include(
    element: &AXUIElement,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> Option<UiElement> {
    let role = get_role(element);
    let valid_role = match role.as_str() {
        "AXApplication" => false,
        "AXMenuItem" => false,
        "AXWindow" => false,  //otherwise highlights entire window
        "AXWebArea" => false, //otherwise highlights entire window for vscode
        "AXOutline" => false, //otherwise highlights sidebar of finder
        "AXGroup" => false,   //normally includes other items
        "AXRow" => false,     //normally includes other items

        //  "AXStaticText" | "AXMenu" | "AXToolbar"  | "AXWebArea"
        // | "AXTabGroup" | "AXOutline" | "AXHeading" => false,

        // "AXRow" | "AXGroup" //vscode uses these two for filelist
        "AXButton" | "AXCheckBox" | "AXRadioButton" | "AXPopUpButton" => true,
        _ => true,
    };
    if !valid_role {
        return None;
    }

    let name = get_name(element).replace(|a: char| !(a.is_alphanumeric() || a.is_whitespace()), "");
    if name.is_empty() {
        // println!("Excluding {} with no name",AXUIElementDisplay(element.clone()));
        return None;
    }
    //check bounds
    let (posx, posy) = get_pos(element);
    let in_bounds = role == "AXMenuBarItem"
        || (posx >= x && posx <= x + width && posy >= y && posy <= y + height);

    if !in_bounds {
        return None;
    }

    Some(into_element(element, name, role, posx, posy))
}

fn must_descend(element: &AXUIElement) -> bool {
    let mut must = true;

    //if has visible children attribute, they must be visible
    if let Ok(vis) = element.visible_children() {
        must &= vis.len() > 0;
    };

    must
}

fn get_name(element: &AXUIElement) -> String {
    let desc = element
        .description()
        .unwrap_or(CFString::from(""))
        .to_string();
    if !desc.is_empty() {
        return desc;
    }
    let title = element.title().unwrap_or(CFString::from("")).to_string();
    if !title.is_empty() {
        return title;
    }
    let role = get_role(element);
    if ["AXTextField", "AXStaticText"].contains(&role.as_str()) {
        if let Ok(val) = element.value() {
            let val = to_contents(val);
            if !val.is_empty() {
                return val;
            }
        }
    }
    String::new()
}

fn get_role(element: &AXUIElement) -> String {
    element.role().unwrap_or(CFString::from("")).to_string()
}
fn get_pos(element: &AXUIElement) -> (i32, i32) {
    let mut pos = (0, 0);
    if let Ok(pos2) = element.attribute(&AXAttribute::new(&CFString::new("AXPosition"))) {
        pos = to_pos(pos2);
    }
    pos
}

fn get_size(element: &AXUIElement) -> (i32, i32) {
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
fn to_contents(cfstr: CFType) -> String {
    let temp: Option<CFString> = cfstr.downcast();
    if let Some(str) = temp {
        return str.to_string();
    } else {
        return String::new();
    }
}

struct AXUIElementDisplay(AXUIElement);

impl Display for AXUIElementDisplay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let chil = if let Ok(ch) = self.0.children() {
            ch.len()
        } else {
            0
        };
        let role = get_role(&self.0);
        writeln!(
            f,
            "name:{} role:'{}'  children - {}",
            get_name(&self.0),
            role,
            chil,
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
