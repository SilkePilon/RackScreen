//! The `face` role: two eyes on a 24×24 dot matrix. Filled in by the face tasks.

use crate::anim::Secs;
use crate::model::Model;
use crate::scene::Scene;

pub fn face_scene(_model: &Model, _now: Secs) -> Scene {
    Scene::new()
}
