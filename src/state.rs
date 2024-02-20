use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use crossbeam_channel::*;

use crate::AIGymSettings;

/// `AIGymStateInner` handles synchronization between the engine thread and the API thread
/// via set of channels. The engine thread will send messages to the API thread and wait for a response.
///
/// (StepRequest, ResetRequest, StepResult, ResetResult) — these are the messages.  Requests are sent
/// from API to the engine. Results are sent from engine to the API once the request is processed.
///
/// Other fields are used to store the state of the environment,
/// plugin settings and gym data tuple (S,A,R,T)
///
/// `AIGymStateInner` is never used directly, instead it's wrapped in `Arc<Mutex<AIGymStateInner>>`
/// and used as resource in bevy systems and parallel-running REST API thread
#[derive(Resource)]
pub struct AIGymStateInner<
    A: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe,
    B: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe,
> {
    // Bevy image handle for the screen
    pub render_image_handles: Vec<Handle<Image>>,

    // Sync with engine thread.
    pub(crate) step_request_tx: Sender<Vec<Option<String>>>,
    pub(crate) step_request_rx: Receiver<Vec<Option<String>>>,

    pub(crate) reset_request_tx: Sender<bool>,
    pub(crate) reset_request_rx: Receiver<bool>,

    pub(crate) step_result_tx: Sender<Vec<bool>>,
    pub(crate) step_result_rx: Receiver<Vec<bool>>,

    pub(crate) reset_result_tx: Sender<bool>,
    pub(crate) reset_result_rx: Receiver<bool>,

    pub(crate) environment_state: Option<B>,

    // Settings
    pub settings: AIGymSettings,

    // State
    pub visual_observations: Vec<image::RgbaImage>,
    pub rewards: Vec<f32>,
    pub actions: Vec<Option<A>>,
    pub terminations: Vec<bool>,
}

impl<
        A: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe,
        B: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe,
    > AIGymStateInner<A, B>
{
    pub fn new(settings: AIGymSettings) -> Self {
        let (step_tx, step_rx) = bounded(1);
        let (reset_tx, reset_rx) = bounded(1);
        let (result_tx, result_rx) = bounded(1);
        let (result_reset_tx, result_reset_rx) = bounded(1);
        Self {
            // Channels
            step_request_tx: step_tx,
            step_request_rx: step_rx,
            step_result_tx: result_tx,
            step_result_rx: result_rx,

            reset_request_tx: reset_tx,
            reset_request_rx: reset_rx,
            reset_result_tx: result_reset_tx,
            reset_result_rx: result_reset_rx,

            environment_state: None,

            // Render Targets
            render_image_handles: Vec::new(),

            // State
            visual_observations: Vec::new(),
            rewards: vec![0.0; settings.num_agents as usize],
            actions: vec![None; settings.num_agents as usize],
            terminations: vec![false; settings.num_agents as usize],

            // Other
            settings,
        }
    }

    // Syncronization happens by sending messages to result-response channels

    /// Once the simulation step is done, send the results back to the API thread
    pub fn send_step_result(&self, results: Vec<bool>) {
        if self.step_result_tx.is_empty() {
            self.step_result_tx.send(results).unwrap();
        }
    }

    /// Once the simulation reset, send the results back to the API thread
    pub fn send_reset_result(&self, result: bool) {
        if self.reset_result_tx.is_empty() {
            self.reset_result_tx.send(result).unwrap();
        }
    }

    /// Recieve serialized actions from the API thread
    pub fn receive_action_strings(&self) -> Vec<Option<String>> {
        self.step_request_rx.recv().unwrap()
    }

    /// Recieve reset request from the API thread
    pub fn receive_reset_request(&self) {
        self.reset_request_rx.recv().unwrap();
    }

    /// Check whether the API thread has sent a step request
    pub fn is_next_action(&self) -> bool {
        !self.step_request_rx.is_empty()
    }

    /// Check whether the API thread has sent a reset request
    pub fn is_reset_request(&self) -> bool {
        !self.reset_request_rx.is_empty()
    }

    /// set_reward is used to set the reward for the agent
    pub fn set_reward(&mut self, agent_index: usize, score: f32) {
        self.rewards[agent_index] = score;
    }

    /// set_terminated is used to mark the agent as terminated
    pub fn set_terminated(&mut self, agent_index: usize, result: bool) {
        self.terminations[agent_index] = result;
    }

    /// reset `bevy_rl` state history (terminated statuses and reward for agents)
    pub fn reset(&mut self) {
        for i in 0..self.terminations.len() {
            self.set_terminated(i, false);
            self.set_reward(i, 0.0);
        }

        self.send_reset_result(true);
    }

    /// set_env_state is used to synchrinize simulation state with bevy_rl for REST API
    pub fn set_env_state(&mut self, state: B) {
        self.environment_state = Some(state);
    }
}

/// `AIGymStateInner` is never used directly, instead it's wrapped
/// in `Arc<Mutex<AIGymStateInner>>` as `AIGymState` and used as resource in bevy systems
/// To use AIGymState you would need to lock it with `AIGymState::lock()`
#[derive(Resource, Deref, DerefMut, Clone)]
pub struct AIGymState<
    A: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe,
    B: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe,
>(pub Arc<Mutex<AIGymStateInner<A, B>>>);

impl<
        A: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe,
        B: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe,
    > AIGymState<A, B>
{
    pub fn new(settings: AIGymSettings) -> Self {
        Self(Arc::new(Mutex::new(AIGymStateInner::new(settings))))
    }
}
