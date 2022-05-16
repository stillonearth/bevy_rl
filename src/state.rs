use image;

use bevy::{prelude::*, render::camera::RenderTarget};
use crossbeam_channel::*;

pub struct AIBrain<A: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe> {
    // Bevy internals
    pub _render_target: Option<RenderTarget>,
    pub _render_image_handle: Option<Handle<Image>>,

    // State
    pub screen: Option<image::RgbaImage>,
    pub reward: f32,
    pub action: Option<A>,
    pub is_terminated: bool,
}

pub struct AIGymState<A: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe> {
    // synchronizing with environment
    pub(crate) __step_channel_tx: Sender<String>,
    pub(crate) __step_channel_rx: Receiver<String>,

    pub(crate) __reset_channel_tx: Sender<bool>,
    pub(crate) __reset_channel_rx: Receiver<bool>,

    pub(crate) __result_channel_tx: Sender<bool>,
    pub(crate) __result_channel_rx: Receiver<bool>,

    pub(crate) __result_reset_channel_tx: Sender<bool>,
    pub(crate) __result_reset_channel_rx: Receiver<bool>,

    pub brains: Vec<AIBrain<A>>,
}

impl<A: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe> AIGymState<A> {
    pub fn new(num_brains: u8) -> Self {
        let (step_tx, step_rx) = bounded(1);
        let (reset_tx, reset_rx) = bounded(1);
        let (result_tx, result_rx) = bounded(1);
        let (result_reset_tx, result_reset_rx) = bounded(1);

        let mut brains: Vec<AIBrain<A>> = Vec::new();

        for _ in 0..num_brains {
            brains.push(AIBrain {
                _render_target: None,
                _render_image_handle: None,
                screen: None,
                reward: 0.0,
                action: None,
                is_terminated: false,
            })
        }

        Self {
            __step_channel_tx: step_tx,
            __step_channel_rx: step_rx,
            __result_channel_tx: result_tx,
            __result_channel_rx: result_rx,
            __reset_channel_tx: reset_tx,
            __reset_channel_rx: reset_rx,
            __result_reset_channel_tx: result_reset_tx,
            __result_reset_channel_rx: result_reset_rx,
            brains: brains,
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

    pub fn set_score(&mut self, brain: usize, score: f32) {
        self.brains[brain].reward = score;
    }

    pub fn set_terminated(&mut self, brain: usize, result: bool) {
        self.brains[brain].is_terminated = result;
    }

    pub fn reset(&mut self) {
        for mut brain in 0..self.brains.len() {
            self.set_terminated(brain, false);
            self.set_score(brain, 0.0);
        }
        self.__result_reset_channel_tx.send(true).unwrap();
    }
}

pub struct EnvironmentState {
    pub is_terminated: bool,
    pub reward: f32,
}
