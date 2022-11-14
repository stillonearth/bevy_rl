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

use image;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::io::Cursor;
use std::sync::{Arc, Mutex};

use crate::{render, state};

#[derive(Serialize, Deserialize)]
pub(crate) struct AgentState {
    reward: f32,
    is_terminated: bool,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct AgentAction {
    action: Option<String>,
}

#[derive(Clone, StateData)]
pub(crate) struct GothamState<
    T: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe,
    P: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe + serde::Serialize,
> {
    pub(crate) inner: Arc<Mutex<state::AIGymStateInner<T, P>>>,
    pub(crate) settings: render::AIGymSettings,
}

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
        route.post("/reset").to(reset::<T, P>);
        route.get("/state").to(env_state::<T, P>);
    })
}

fn visual_observations<
    T: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe,
    P: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe + serde::Serialize,
>(
    state: State,
) -> (State, Response<Body>) {
    let screens: Vec<image::RgbaImage>;
    let settings: render::AIGymSettings;
    {
        let state_: &GothamState<T, P> = GothamState::borrow_from(&state);
        let state__ = state_.inner.lock().unwrap();
        screens = state__.visual_observations.clone();
        settings = state_.settings.clone();
    }

    let mut bytes: Vec<u8> = Vec::new();
    let mut all_agents_image =
        image::RgbaImage::new(settings.width * settings.num_agents, settings.height);
    let mut agent_index = 0;

    for screen in screens.iter() {
        let image = screen.clone();

        image::imageops::overlay(
            &mut all_agents_image,
            &image,
            (agent_index * settings.width) as i64,
            0,
        );

        agent_index += 1;
    }

    all_agents_image
        .write_to(&mut Cursor::new(&mut bytes), image::ImageOutputFormat::Png)
        .unwrap();

    let response = create_response::<Vec<u8>>(&state, StatusCode::OK, mime::IMAGE_PNG, bytes);

    return (state, response);
}

#[derive(Deserialize, StateData, StaticResponseExtender)]
struct StepQueryString {
    payload: String,
}

fn step<
    T: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe,
    P: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe + serde::Serialize,
>(
    mut state: State,
) -> (State, String) {
    let query_param = StepQueryString::take_from(&mut state);

    let err = serde_json::from_str::<Vec<AgentAction>>(&query_param.payload).err();
    if err.is_some() {
        return (state, err.unwrap().to_string());
    }
    let agent_actions: Vec<AgentAction> = serde_json::from_str(&query_param.payload).unwrap();

    let state_: &GothamState<T, P> = GothamState::borrow_from(&state);
    let step_tx: Sender<Vec<Option<String>>>;
    let result_rx: Receiver<Vec<bool>>;

    if agent_actions.len() != state_.settings.num_agents as usize {
        return (state, "Invalid number of actions".to_string());
    }

    {
        let ai_gym_state = state_.inner.lock().unwrap();
        step_tx = ai_gym_state._step_tx.clone();
        result_rx = ai_gym_state._step_result_rx.clone();
    }

    let actions = agent_actions
        .iter()
        .map(|agent_action| agent_action.action.clone())
        .collect();

    step_tx.send(actions).unwrap();
    result_rx.recv().unwrap();

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

    return (state, json!(agent_states).to_string());
}

fn reset<
    T: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe,
    P: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe + serde::Serialize,
>(
    state: State,
) -> (State, String) {
    let reset_channel_tx: Sender<bool>;
    let reset_result_channel_rx: Receiver<bool>;
    {
        let state_: &GothamState<T, P> = GothamState::borrow_from(&state);
        let ai_gym_state = state_.inner.lock().unwrap();
        reset_channel_tx = ai_gym_state._reset_tx.clone();
        reset_result_channel_rx = ai_gym_state._reset_result_rx.clone();
    }

    reset_channel_tx.send(true).unwrap();
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

    return (state, json!(agent_states).to_string());
}

fn env_state<
    T: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe,
    P: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe + serde::Serialize,
>(
    state: State,
) -> (State, String) {
    let state_: &GothamState<T, P> = GothamState::borrow_from(&state);
    let env_state = state_.inner.lock().unwrap()._environment_state.clone();

    return (state, json!(env_state).to_string());
}
