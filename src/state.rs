use image;

use bevy::{prelude::*, render::camera::RenderTarget};
use crossbeam_channel::*;

#[derive(Clone)]
pub struct AIGymState<A: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe> {
    // These parts are made of hack trick internals.
    pub __render_target: Option<RenderTarget>, // render target for camera -- window on in our case texture
    pub __render_image_handle: Option<Handle<Image>>, // handle to image we use in bevy UI building.
    // actual texture is GPU ram and we can't access it easily

    // synchronizing with environment
    pub __step_channel_tx: Sender<String>,
    pub __step_channel_rx: Receiver<String>,

    pub __reset_channel_tx: Sender<bool>,
    pub __reset_channel_rx: Receiver<bool>,

    pub __result_channel_tx: Sender<bool>,
    pub __result_channel_rx: Receiver<bool>,

    pub __result_reset_channel_tx: Sender<bool>,
    pub __result_reset_rx: Receiver<bool>,

    // State
    pub screen: Option<image::RgbaImage>,
    pub rewards: Vec<f32>,
    pub action: Option<A>,
    pub is_terminated: bool,
}

impl<A: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe> Default for AIGymState<A> {
    fn default() -> Self {
        let (step_tx, step_rx) = bounded(1);
        let (reset_tx, reset_rx) = bounded(1);
        let (result_tx, result_rx) = bounded(1);
        let (result_reset_tx, result_reset_rx) = bounded(1);
        Self {
            __step_channel_tx: step_tx,
            __step_channel_rx: step_rx,
            __result_channel_tx: result_tx,
            __result_channel_rx: result_rx,
            __reset_channel_tx: reset_tx,
            __reset_channel_rx: reset_rx,
            __render_target: None,
            __render_image_handle: None,
            __result_reset_channel_tx: result_reset_tx,
            __result_reset_rx: result_reset_rx,
            screen: None,
            rewards: Vec::new(),
            action: None,
            is_terminated: false,
        }
    }
}

pub struct EnvironmentState {
    pub is_terminated: bool,
    pub reward: f32,
}
