use std::cell::Cell;

use accessibility::{
    AXAttribute, AXUIElement, AXUIElementAttributes, ElementFinder, TreeVisitor, TreeWalkerFlow,
};
use active_win_pos_rs::get_active_window;
use core_foundation::{
    array::CFArray,
    base::{CFType, TCFType},
    number::CFNumber,
    string::CFString,
};
use easier::prelude::ToCollectionIteratorExtension;

use crate::traits::{AccessibilityCalls, Action, UiElement};

pub struct Osx {
    topmost_pid: Option<i32>,
}

impl Osx {
    pub fn new() -> Self {
        Self { topmost_pid: None }
    }
}
impl AccessibilityCalls for Osx {
    fn get_elements(&self) -> Vec<UiElement> {
        if self.topmost_pid.is_none() {
            return vec![];
        }

        let walker = accessibility::TreeWalker::new();
        let visitor = MyVisitor::new_with_indentation(4);
        //let root = accessibility::ui_element::AXUIElement::system_wide();
        let els = accessibility::ui_element::AXUIElement::application(self.topmost_pid.unwrap());
        walker.walk(&els, &visitor);

        visitor.elements.take()
    }

    fn invoke(&self, element: &UiElement, action: Action) {
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
        let win = get_active_window();
        if let Ok(win) = win {
            self.topmost_pid = Some(win.process_id as i32);
        }
        println!("active pid: {:?}", self.topmost_pid);
    }

    fn has_permissions(&self) -> bool {
        macos_accessibility_client::accessibility::application_is_trusted_with_prompt()
    }
}

struct MyVisitor {
    level: Cell<usize>,
    elements: Cell<Vec<UiElement>>,
    pid: Cell<i32>,
}

impl MyVisitor {
    pub fn new_with_indentation(pid: i32) -> Self {
        Self {
            level: Cell::new(0),
            elements: Cell::new(vec![]),
            pid: Cell::new(pid),
        }
    }
}

impl TreeVisitor for MyVisitor {
    fn enter_element(&self, element: &AXUIElement) -> TreeWalkerFlow {
        self.level.replace(self.level.get() + 1);

        if must_include(element) {
            let mut uie: UiElement = element.into();
            //println!("{uie:?}");
            uie.pid = self.pid.get();
            let mut old = self.elements.take();
            old.push(uie);
            self.elements.set(old);

            let role = element.role().unwrap_or_else(|_| CFString::new(""));
            let title = element.title().unwrap_or_else(|_| CFString::new(""));

            let vis = if let Ok(vis) = element.visible_children() {
                vis.len()
            } else {
                0
            };
            println!["__{:?}- {:?} visi{vis} ", title, role,];

            if let Ok(names) = element.attribute_names() {
                for name in names.into_iter() {
                    /*if &*name == self.children.as_CFString() {
                        continue;
                    }*/

                    /*if let Ok(value) = element.attribute(&AXAttribute::new(&*name)) {
                        println!["<<<<<{}|.  {:?}", *name, value];
                    }*/
                }
            }
            if vis == 0 {
                return TreeWalkerFlow::SkipSubtree;
            }
        }
        TreeWalkerFlow::Continue
    }

    fn exit_element(&self, _element: &AXUIElement) {
        self.level.replace(self.level.get() - 1);
    }
}

impl From<&AXUIElement> for UiElement {
    fn from(value: &AXUIElement) -> Self {
        let pos = format!(
            "{:?}",
            value.attribute(&AXAttribute::new(&CFString::new("AXPosition")))
        );

        let mut pos = (0, 0);
        if let Ok(pos2) = value.attribute(&AXAttribute::new(&CFString::new("AXPosition"))) {
            //println!("POS: {pos2:?}");
            pos = to_pos(pos2);
            //println!("POS@:{a:?}");
        }
        let title = value.title().unwrap_or(CFString::from("")).to_string();
        let desc = value
            .description()
            .unwrap_or(CFString::from(""))
            .to_string();
        let text = if desc.is_empty() { title } else { desc };

        UiElement {
            id: "".to_string(),
            name: text,
            x: pos.0,
            y: pos.1,
            width: 0,
            height: 0,
            control: value
                .role()
                .unwrap_or(CFString::from("Unknown"))
                .to_string(),
            item: "".to_string(), //value                .label_value()                .unwrap_or(CFString::from(""))                .to_string(),
            class: value.identifier().unwrap_or(CFString::from("")).to_string(),
            pid: 0,
            parent: "".to_string(),
        }
    }
}

fn must_include(ele: &AXUIElement) -> bool {
    let mut incl = true;
    if let Ok(acts) = ele.action_names() {
        incl &= acts
            .iter()
            .map(|a| a.to_string())
            .filter(|a| a == "AXPress")
            .count()
            > 0;
    }
    incl &= match ele
        .role()
        .unwrap_or(CFString::from("Unknown"))
        .to_string()
        .as_str()
    {
        "AXGroup" | "AXStaticText" | "AXMenu" | "AXToolbar" | "AXApplication" => false,
        "AXMenuItem" => true,
        _ => true,
    };
    incl
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
