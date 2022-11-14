#![feature(associated_type_bounds)]

use bevy::prelude::Resource;

mod api;
pub mod render;
pub mod state;

#[derive(Clone, Resource)]
pub struct AIGymSettings {
    pub width: u32,
    pub height: u32,
    pub num_agents: u32,
}
