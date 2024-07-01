//! REST API for bevy_rl
//! This module uses gotham web framework to expose REST API for bevy_rl
//! One catch choosing a web framework for Rust here is that it should run without an async runtime
//! and be able to run in a separate thread. Gotham is one of the few web frameworks that can do
//! that from the ones I've tested.
//!
//! Sergei Surovsev <ssurovsev@gmail.com>

use crossbeam_channel::*;

use gotham::helpers::http::response::create_response;
use gotham::middleware::state::StateMiddleware;
use gotham::pipeline::{single_middleware, single_pipeline};
use gotham::prelude::StaticResponseExtender;
use gotham::router::builder::*;
use gotham::router::Router;
use gotham::state::StateData;
use gotham::state::{FromState, State};
use hyper::{Body, Response, StatusCode};

use serde::{Deserialize, Serialize};
use serde_json::json;
use std::io::Cursor;

use crate::{state, AIGymSettings};

/// A reprsentation of agent's state (reward, terminated) in terms of bevy_rl
/// That's not the same as the state of the environment
#[derive(Serialize, Deserialize)]
pub(crate) struct AgentState {
    reward: f32,
    is_terminated: bool,
}

/// This is used for deserializing agent's action from the request body
#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct AgentAction {
    action: Option<String>,
}

/// `GothamState` is a wrapper around `AIGymState` that is used by Gotham middleware
/// It's holds a state of the environment and settings
#[derive(Clone, StateData)]
pub(crate) struct GothamState<
    T: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe,
    P: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe + serde::Serialize,
> {
    pub(crate) inner: state::AIGymState<T, P>,
    pub(crate) settings: AIGymSettings,
}

/// Describes REST API routes
pub(crate) fn router<
    T: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe,
    P: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe + serde::Serialize,
>(
    state: GothamState<T, P>,
) -> Router {
    let middleware = StateMiddleware::new(state);
    let pipeline = single_middleware(middleware);

    let (chain, pipelines) = single_pipeline(pipeline);

    build_router(chain, pipelines, |route| {
        route
            .get("/visual_observations")
            .to(visual_observations::<T, P>);
        route
            .get("/step")
            .with_query_string_extractor::<StepQueryString>()
            .to(step::<T, P>);
        route.get("/reset").to(reset::<T, P>);
        route.get("/state").to(env_state::<T, P>);
    })
}

/// Return rendered visual observations as a single PNG image
fn visual_observations<
    T: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe,
    P: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe + serde::Serialize,
>(
    state: State,
) -> (State, Response<Body>) {
    let screens: Vec<image::RgbaImage>;
    let settings: AIGymSettings;
    {
        let state_: &GothamState<T, P> = GothamState::borrow_from(&state);
        let state__ = state_.inner.lock().unwrap();
        screens = state__.visual_observations.clone();
        settings = state_.settings.clone();
    }

    let mut bytes: Vec<u8> = Vec::new();
    let mut all_agents_image =
        image::RgbaImage::new(settings.width * settings.num_agents, settings.height);

    for (agent_index, screen) in screens.iter().enumerate() {
        let image = screen.clone();

        image::imageops::overlay(
            &mut all_agents_image,
            &image,
            ((agent_index as u32) * settings.width) as i64,
            0,
        );
    }

    all_agents_image
        .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
        .unwrap();

    let response = create_response::<Vec<u8>>(&state, StatusCode::OK, mime::IMAGE_PNG, bytes);

    (state, response)
}

/// Describe the query string for the step request
#[derive(Deserialize, StateData, StaticResponseExtender)]
struct StepQueryString {
    payload: String,
}

/// `step` API endpoint to take an action and return the next `AgentState`
fn step<
    T: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe,
    P: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe + serde::Serialize,
>(
    mut state: State,
) -> (State, String) {
    let query_param = StepQueryString::take_from(&mut state);

    let err = serde_json::from_str::<Vec<AgentAction>>(&query_param.payload).err();
    if let Some(message) = err {
        return (state, message.to_string());
    }
    let agent_actions: Vec<AgentAction> = serde_json::from_str(&query_param.payload).unwrap();

    let state_: &GothamState<T, P> = GothamState::borrow_from(&state);
    let step_request_tx: Sender<Vec<Option<String>>>;
    let setp_result_rx: Receiver<Vec<bool>>;

    if agent_actions.len() != state_.settings.num_agents as usize {
        return (state, "Invalid number of actions".to_string());
    }

    {
        let ai_gym_state = state_.inner.lock().unwrap();
        step_request_tx = ai_gym_state.step_request_tx.clone();
        setp_result_rx = ai_gym_state.step_result_rx.clone();
    }

    let actions = agent_actions
        .iter()
        .map(|agent_action| agent_action.action.clone())
        .collect();

    step_request_tx.send(actions).unwrap();
    setp_result_rx.recv().unwrap();

    let mut agent_states: Vec<AgentState> = Vec::new();
    {
        let ai_gym_state = state_.inner.lock().unwrap();
        for i in 0..ai_gym_state.rewards.len() {
            agent_states.push(AgentState {
                reward: ai_gym_state.rewards[i],
                is_terminated: ai_gym_state.terminations[i],
            });
        }
    }

    (state, json!(agent_states).to_string())
}

/// `reset` API endpoint to reset the environment
fn reset<
    T: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe,
    P: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe + serde::Serialize,
>(
    state: State,
) -> (State, String) {
    let reset_request_channel_tx: Sender<bool>;
    let reset_result_channel_rx: Receiver<bool>;
    {
        let state_: &GothamState<T, P> = GothamState::borrow_from(&state);
        let ai_gym_state = state_.inner.lock().unwrap();
        reset_request_channel_tx = ai_gym_state.reset_request_tx.clone();
        reset_result_channel_rx = ai_gym_state.reset_result_rx.clone();
    }

    reset_request_channel_tx.send(true).unwrap();
    reset_result_channel_rx.recv().unwrap();

    let state_: &GothamState<T, P> = GothamState::borrow_from(&state);
    let mut agent_states: Vec<AgentState> = Vec::new();
    {
        let ai_gym_state = state_.inner.lock().unwrap();
        for i in 0..ai_gym_state.rewards.len() {
            agent_states.push(AgentState {
                reward: ai_gym_state.rewards[i],
                is_terminated: ai_gym_state.terminations[i],
            });
        }
    }

    (state, json!(agent_states).to_string())
}

/// `env_state` API endpoint to get the environment state
fn env_state<
    T: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe,
    P: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe + serde::Serialize,
>(
    state: State,
) -> (State, String) {
    let state_: &GothamState<T, P> = GothamState::borrow_from(&state);
    let env_state = state_.inner.lock().unwrap().environment_state.clone();

    (state, json!(env_state).to_string())
}
