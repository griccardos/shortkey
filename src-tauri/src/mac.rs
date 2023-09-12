use crate::traits::{AccessibilityCalls, Action, UiElement};

pub struct Osx;

impl Osx {
    pub fn new() -> Self {
        Self
    }
}
impl AccessibilityCalls for Osx {
    fn get_elements(&self) -> Vec<UiElement> {
        todo!()
    }

    fn invoke(&self, element: &UiElement, action: Action) {
        todo!()
    }

    fn save_topmost(&mut self) {
        todo!()
    }
}
