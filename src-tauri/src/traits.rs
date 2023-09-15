use serde::{Deserialize, Serialize};

pub trait AccessibilityCalls {
    ///check if has permissions
    fn has_permissions(&self) -> bool;
    ///get the elements which we can click on
    fn get_elements(&self) -> Vec<UiElement>;
    ///do the click event
    fn invoke(&self, element: &UiElement, action: Action);
    ///we must call this before displaying the window
    fn save_topmost(&mut self);
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Action {
    LeftClick,
    RightClick,
}
#[derive(Clone, Debug)]
pub struct UiElement {
    pub id: String,
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub control: String,
    pub item: String,
    pub class: String,
    pub pid: i32,
    pub parent: String,
    pub x_offset: i32,
    pub y_offset: i32,
}
