pub mod calibrate;
pub mod configure;
pub mod install;
pub mod menu;
pub mod run;
// `screens::screens` is the Screens editor; the repeated name is the screen's own name.
#[allow(clippy::module_inception)]
pub mod screens;
pub mod status;
pub mod uninstall;

use crate::{Screen, ScreenId, Shared};

pub fn make(id: ScreenId, shared: &Shared) -> Box<dyn Screen> {
    match id {
        ScreenId::Menu => Box::new(menu::Menu::new(shared)),
        ScreenId::Install => Box::new(install::Install::new(shared)),
        ScreenId::Calibrate => Box::new(calibrate::Calibrate::new(shared)),
        ScreenId::Screens => Box::new(screens::Screens::new(shared)),
        ScreenId::Configure => Box::new(configure::Configure::new(shared)),
        ScreenId::Status => Box::new(status::Status::new(shared)),
        ScreenId::RunHere => Box::new(run::RunHere::new(shared)),
        ScreenId::Uninstall => Box::new(uninstall::Uninstall::new(shared)),
    }
}
