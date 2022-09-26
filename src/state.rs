use image;

use bevy::prelude::*;
use crossbeam_channel::*;

use crate::AIGymSettings;
pub struct AIGymState<A: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe> {
    // Bevy image handle for the screen
    pub render_image_handles: Vec<Handle<Image>>,

    // Sync with engine thread.
    pub(crate) _step_tx: Sender<Vec<Option<String>>>,
    pub(crate) _step_rx: Receiver<Vec<Option<String>>>,

    pub(crate) _reset_tx: Sender<bool>,
    pub(crate) _reset_rx: Receiver<bool>,

    pub(crate) _step_result_tx: Sender<Vec<bool>>,
    pub(crate) _step_result_rx: Receiver<Vec<bool>>,

    pub(crate) _reset_result_tx: Sender<bool>,
    pub(crate) _reset_result_rx: Receiver<bool>,

    // State
    pub screens: Vec<image::RgbaImage>,
    pub rewards: Vec<f32>,
    pub actions: Vec<Option<A>>,
    pub terminations: Vec<bool>,
}

impl<A: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe> AIGymState<A> {
    pub fn new(settings: AIGymSettings) -> Self {
        let (step_tx, step_rx) = bounded(1);
        let (reset_tx, reset_rx) = bounded(1);
        let (result_tx, result_rx) = bounded(1);
        let (result_reset_tx, result_reset_rx) = bounded(1);
        Self {
            // Channels
            _step_tx: step_tx,
            _step_rx: step_rx,
            _step_result_tx: result_tx,
            _step_result_rx: result_rx,

            _reset_tx: reset_tx,
            _reset_rx: reset_rx,
            _reset_result_tx: result_reset_tx,
            _reset_result_rx: result_reset_rx,

            // Render Targets
            render_image_handles: Vec::new(),

            // State
            screens: Vec::new(),
            rewards: vec![0.0; settings.num_agents as usize],
            actions: vec![None; settings.num_agents as usize],
            terminations: vec![false; settings.num_agents as usize],
        }
    }

    pub fn send_step_result(&self, results: Vec<bool>) {
        if self._step_result_tx.is_empty() {
            self._step_result_tx.send(results).unwrap();
        }
    }

    pub fn send_reset_result(&self, result: bool) {
        if self._reset_result_tx.is_empty() {
            self._reset_result_tx.send(result).unwrap();
        }
    }

    pub fn receive_action_strings(&self) -> Vec<Option<String>> {
        return self._step_rx.recv().unwrap();
    }

    pub fn receive_reset_request(&self) {
        self._reset_rx.recv().unwrap();
    }

    pub fn is_next_action(&self) -> bool {
        return !self._step_rx.is_empty();
    }

    pub fn is_reset_request(&self) -> bool {
        return !self._reset_rx.is_empty();
    }

    pub fn set_reward(&mut self, agent_index: usize, score: f32) {
        self.rewards[agent_index] = score;
    }

    pub fn set_terminated(&mut self, agent_index: usize, result: bool) {
        self.terminations[agent_index] = result;
    }

    pub fn reset(&mut self) {
        for i in 0..self.terminations.len() {
            self.set_terminated(i, false);
            self.set_reward(i, 0.0);
        }

        self.send_reset_result(true);
    }
}
