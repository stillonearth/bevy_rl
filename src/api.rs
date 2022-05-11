use crossbeam_channel::*;

use gotham::helpers::http::response::create_response;
use gotham::middleware::state::StateMiddleware;
use gotham::pipeline::{single_middleware, single_pipeline};
use gotham::router::builder::*;
use gotham::router::Router;
use gotham::state::StateData;
use gotham::state::{FromState, State};
use hyper::{body, Body, Response, StatusCode};

use futures::executor;
use image;
use std::io::Cursor;
use std::sync::{Arc, Mutex};

use crate::state;

// ---------------
// AI Gym REST API
// ---------------

#[derive(Clone, StateData)]
pub(crate) struct GothamState<T: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe> {
    pub(crate) inner: Arc<Mutex<state::AIGymState<T>>>,
}

pub(crate) fn router<T: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe>(
    state: GothamState<T>,
) -> Router {
    let middleware = StateMiddleware::new(state);
    let pipeline = single_middleware(middleware);

    let (chain, pipelines) = single_pipeline(pipeline);

    // build a router with the chain & pipeline
    build_router(chain, pipelines, |route| {
        route.get("/screen.png").to(screen::<T>);
        route.post("/step").to(step::<T>);
        route.post("/reset").to(reset::<T>);
    })
}

fn screen<T: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe>(
    state: State,
) -> (State, Response<Body>) {
    let mut bytes: Vec<u8> = Vec::new();
    {
        let state_: &GothamState<T> = GothamState::borrow_from(&state);
        let state__ = state_.inner.lock().unwrap();

        if !state__.screen.is_none() {
            let image = state__.screen.clone().unwrap();

            image
                .write_to(&mut Cursor::new(&mut bytes), image::ImageOutputFormat::Png)
                .unwrap();
        }
    }
    let response = create_response::<Vec<u8>>(&state, StatusCode::OK, mime::TEXT_PLAIN, bytes);

    return (state, response);
}

fn step<T: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe>(
    mut state: State,
) -> (State, String) {
    let body_ = Body::take_from(&mut state);
    let valid_body = executor::block_on(body::to_bytes(body_)).unwrap();
    let action = String::from_utf8(valid_body.to_vec()).unwrap();

    let state_: &GothamState<T> = GothamState::borrow_from(&state);
    let step_tx: Sender<String>;
    let result_rx: Receiver<bool>;

    let is_terminated: bool;
    {
        let ai_gym_state = state_.inner.lock().unwrap();
        is_terminated = ai_gym_state.is_terminated;
        step_tx = ai_gym_state.__step_channel_tx.clone();
        result_rx = ai_gym_state.__result_channel_rx.clone();
    }

    if is_terminated {
        return (
            state,
            format!("{{\"reward\": {}, \"is_terminated\": {}}}", 0, true),
        );
    }

    step_tx.send(action).unwrap();
    result_rx.recv().unwrap();

    let reward;
    let is_terminated;
    {
        let ai_gym_state = state_.inner.lock().unwrap();

        reward = ai_gym_state.reward;
        is_terminated = ai_gym_state.is_terminated.clone();
    }

    return (
        state,
        format!(
            "{{\"reward\": {}, \"is_terminated\": {}}}",
            reward, is_terminated
        ),
    );
}

fn reset<T: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe>(
    state: State,
) -> (State, String) {
    let reset_channel_tx: Sender<bool>;
    let reset_result_channel_rx: Receiver<bool>;
    {
        let state_: &GothamState<T> = GothamState::borrow_from(&state);
        let ai_gym_state = state_.inner.lock().unwrap();
        reset_channel_tx = ai_gym_state.__reset_channel_tx.clone();
        reset_result_channel_rx = ai_gym_state.__result_reset_channel_rx.clone();
    }

    reset_channel_tx.send(true).unwrap();
    reset_result_channel_rx.recv().unwrap();

    return (state, "ok".to_string());
}
