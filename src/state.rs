use image;

use bevy::{prelude::*, render::camera::RenderTarget};
use crossbeam_channel::*;

// #[derive(Clone)]
pub struct AIGymState<A: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe> {
    // These parts are made of hack trick internals.
    pub render_image_handles: Vec<Handle<Image>>,

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
    pub screen: Vec<image::RgbaImage>,
    pub reward: f32,
    pub action: Option<A>,
    pub is_terminated: bool,
}

impl<A: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe> AIGymState<A> {
    pub fn new() -> Self {
        let (step_tx, step_rx) = bounded(1);
        let (reset_tx, reset_rx) = bounded(1);
        let (result_tx, result_rx) = bounded(1);
        let (result_reset_tx, result_reset_rx) = bounded(1);
        Self {
            // Channels
            __step_channel_tx: step_tx,
            __step_channel_rx: step_rx,
            __result_channel_tx: result_tx,
            __result_channel_rx: result_rx,
            __reset_channel_tx: reset_tx,
            __reset_channel_rx: reset_rx,
            __result_reset_channel_tx: result_reset_tx,
            __result_reset_channel_rx: result_reset_rx,

            // Render Targets
            render_image_handles: Vec::new(),

            // State
            screen: Vec::new(),
            reward: 0.0,
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
        self.reward = score;
    }

    pub fn set_terminated(&mut self, result: bool) {
        self.is_terminated = result;
    }

    pub fn reset(&mut self) {
        self.set_terminated(false);
        self.reward = 0.0;
        self.__result_reset_channel_tx.send(true).unwrap();
    }
}

pub struct EnvironmentState {
    pub is_terminated: bool,
    pub reward: f32,
}
