#![feature(associated_type_bounds)]

use bevy::prelude::Resource;

mod api;
pub mod render;
pub mod state;

pub use render::*;
pub use state::*;

#[derive(Clone, Resource, Default)]
pub struct AIGymSettings {
    pub width: u32,
    pub height: u32,
    pub num_agents: u32,
    /// Ignore rending buffer
    pub render_to_buffer: bool,
}
