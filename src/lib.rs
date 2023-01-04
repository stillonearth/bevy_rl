#![feature(associated_type_bounds)]

use bevy::{
    prelude::*,
    render::{RenderApp, RenderStage},
    time::{Timer, TimerMode},
};

mod api;
pub mod render;
pub mod state;

pub use render::*;
pub use state::*;

pub type Callback = fn();

#[derive(Clone, Resource, Default)]
pub struct AIGymSettings {
    pub width: u32,
    pub height: u32,
    pub num_agents: u32,
    pub pause_interval: f32,

    // Ignore rending buffer
    pub render_to_buffer: bool,
}

pub struct EventReset;

pub struct EventControl(pub Vec<Option<String>>);

pub struct EventPauseResume;

#[derive(Debug, Clone, Eq, PartialEq, Hash, Resource)]
pub enum SimulationState {
    Running,
    PausedForControl,
}

#[derive(Resource)]
pub struct SimulationPauseTimer(Timer);

impl<
        T: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe,
        P: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe + serde::Serialize,
    > Plugin for AIGymPlugin<T, P>
{
    fn build(&self, app: &mut App) {
        app.add_startup_system(setup_render_app::<T, P>.label("setup_rendering"));

        let ai_gym_state = app
            .world
            .get_resource::<state::AIGymState<T, P>>()
            .unwrap()
            .clone();

        {
            let ai_gym_state = ai_gym_state.lock().unwrap();
            if !ai_gym_state.settings.render_to_buffer {
                return;
            }

            app.insert_resource(SimulationPauseTimer(Timer::from_seconds(
                ai_gym_state.settings.pause_interval,
                TimerMode::Repeating,
            )));
        }

        // Add system scheduling
        app.add_system_set(
            SystemSet::on_update(SimulationState::Running).with_system(control_switch::<T, P>),
        );

        app.add_system_set(
            SystemSet::on_update(SimulationState::PausedForControl)
                // Game Systems
                .with_system(process_control_request::<T, P>) // System that parses user command
                .with_system(process_reset_request::<T, P>), // System that performs environment state reset
        );

        if let Ok(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.add_system_to_stage(RenderStage::Render, copy_from_gpu_to_ram::<T, P>);
            render_app.insert_resource(ai_gym_state);
        }
    }
}

// Pausing the external world i neach tick
fn control_switch<
    T: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe,
    P: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe + serde::Serialize,
>(
    mut app_state: ResMut<State<SimulationState>>,
    time: Res<Time>,
    mut timer: ResMut<SimulationPauseTimer>,
    ai_gym_state: ResMut<state::AIGymState<T, P>>,
    mut pause_event_writer: EventWriter<EventPauseResume>,
) {
    let ai_gym_settings = ai_gym_state.lock().unwrap().settings.clone();
    // This controls control frequency of the environment
    if timer.0.tick(time.delta()).just_finished() {
        // Set current state to control to disable simulation systems
        app_state
            .overwrite_push(SimulationState::PausedForControl)
            .unwrap();

        // Pause time in all environment
        pause_event_writer.send(EventPauseResume);

        // ai_gym_state is behind arc mutex, so we need to lock it
        let ai_gym_state = ai_gym_state.lock().unwrap();

        // This will tell bevy_rl that environment is ready to receive actions
        let results = (0..ai_gym_settings.num_agents).map(|_| true).collect();
        ai_gym_state.send_step_result(results);
    }
}

pub(crate) fn process_reset_request<
    T: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe,
    P: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe + serde::Serialize,
>(
    ai_gym_state: ResMut<state::AIGymState<T, P>>,
    mut reset_event_writer: EventWriter<EventReset>,
) {
    let ai_gym_state = ai_gym_state.lock().unwrap();
    if !ai_gym_state.is_reset_request() {
        return;
    }

    ai_gym_state.receive_reset_request();
    reset_event_writer.send(EventReset);
}

pub(crate) fn process_control_request<
    T: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe,
    P: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe + serde::Serialize,
>(
    mut app_state: ResMut<State<SimulationState>>,
    ai_gym_state: ResMut<state::AIGymState<T, P>>,
    mut reset_event_writer: EventWriter<EventControl>,
) {
    let ai_gym_state = ai_gym_state.lock().unwrap();

    // Drop the system if users hasn't sent request this frame
    if !ai_gym_state.is_next_action() {
        return;
    }

    let unparsed_actions = ai_gym_state.receive_action_strings();
    reset_event_writer.send(EventControl(unparsed_actions));

    app_state.pop().unwrap();
}
