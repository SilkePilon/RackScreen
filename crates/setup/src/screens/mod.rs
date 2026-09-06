pub mod install;
pub mod menu;
pub mod placeholder;
pub mod uninstall;

use crate::{Screen, ScreenId, Shared};

pub fn make(id: ScreenId, shared: &Shared) -> Box<dyn Screen> {
    match id {
        ScreenId::Menu => Box::new(menu::Menu::new(shared)),
        ScreenId::Install => Box::new(install::Install::new(shared)),
        ScreenId::Uninstall => Box::new(uninstall::Uninstall::new(shared)),
        other => Box::new(placeholder::Placeholder::new(other)),
    }
}
