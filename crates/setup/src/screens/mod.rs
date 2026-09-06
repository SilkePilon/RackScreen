pub mod menu;
pub mod placeholder;

use crate::{Screen, ScreenId, Shared};

pub fn make(id: ScreenId, shared: &Shared) -> Box<dyn Screen> {
    match id {
        ScreenId::Menu => Box::new(menu::Menu::new(shared)),
        other => Box::new(placeholder::Placeholder::new(other)),
    }
}
