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
    pub(crate) __step_channel_tx: Sender<String>,
    pub(crate) __step_channel_rx: Receiver<String>,

    pub(crate) __reset_channel_tx: Sender<bool>,
    pub(crate) __reset_channel_rx: Receiver<bool>,

    pub(crate) __result_channel_tx: Sender<bool>,
    pub(crate) __result_channel_rx: Receiver<bool>,

    pub(crate) __result_reset_channel_tx: Sender<bool>,
    pub(crate) __result_reset_channel_rx: Receiver<bool>,

    // State
    pub screen: Option<image::RgbaImage>,
    pub rewards: Vec<f32>,
    pub action: Option<A>,
    pub(crate) is_terminated: bool,
}

impl<A: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe> AIGymState<A> {
    pub fn new() -> Self {
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
            __result_reset_channel_rx: result_reset_rx,
            screen: None,
            rewards: Vec::new(),
            action: None,
            is_terminated: false,
        }
    }

    pub fn send_step_result(&self, result: bool) {
        if self.__result_channel_tx.is_empty() {
            self.__result_channel_tx.send(result).unwrap();
        }
    }

    pub fn send_reset_result(&self, result: bool) {
        if self.__reset_channel_tx.is_empty() {
            self.__reset_channel_tx.send(result).unwrap();
        }
    }

    pub fn receive_action_string(&self) -> String {
        return self.__step_channel_rx.recv().unwrap();
    }

    pub fn receive_reset_request(&self) {
        self.__reset_channel_rx.recv().unwrap();
    }

    pub fn is_next_action(&self) -> bool {
        return !self.__step_channel_rx.is_empty();
    }

    pub fn is_reset_request(&self) -> bool {
        return !self.__reset_channel_tx.is_empty();
    }

    pub fn set_score(&mut self, score: f32) {
        self.rewards.push(score);
    }

    pub fn set_terminated(&mut self, result: bool) {
        self.is_terminated = result;
    }

    pub fn reset(&mut self) {
        self.set_terminated(false);
        self.rewards = Vec::new();
        self.__result_reset_channel_tx.send(true).unwrap();
    }
}

pub struct EnvironmentState {
    pub is_terminated: bool,
    pub reward: f32,
}
