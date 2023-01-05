#![feature(associated_type_bounds)]

use std::{marker::PhantomData, thread};

use bevy::{
    prelude::*,
    render::{view::RenderLayers, RenderApp, RenderStage},
    time::{Timer, TimerMode},
};

mod api;
pub mod render;
pub mod state;

use render::copy_from_gpu_to_ram;
pub use state::*;
use wgpu::{Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages};

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

#[derive(Default, Clone)]
pub struct AIGymPlugin<
    T: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe,
    P: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe + serde::Serialize,
>(pub PhantomData<(T, P)>);

impl<
        T: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe,
        P: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe + serde::Serialize,
    > Plugin for AIGymPlugin<T, P>
{
    fn build(&self, app: &mut App) {
        app.add_startup_system(setup::<T, P>.label("bevy_rl_setup_rendering"));

        let ai_gym_state = app
            .world
            .get_resource::<state::AIGymState<T, P>>()
            .unwrap()
            .clone();

        {
            let ai_gym_state = ai_gym_state.lock().unwrap();
            app.insert_resource(SimulationPauseTimer(Timer::from_seconds(
                ai_gym_state.settings.pause_interval,
                TimerMode::Repeating,
            )));
        }

        // Initial state
        app.add_state(SimulationState::Running);

        // Register events
        app.add_event::<EventReset>();
        app.add_event::<EventControl>();
        app.add_event::<EventPauseResume>();

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

pub(crate) fn setup<
    T: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe,
    P: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe + serde::Serialize,
>(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    ai_gym_state: ResMut<state::AIGymState<T, P>>,
    mut windows: ResMut<Windows>,
) {
    let ai_gym_state_locked = ai_gym_state.into_inner().clone();
    let mut ai_gym_state = ai_gym_state_locked.lock().unwrap();
    let ai_gym_settings = ai_gym_state.settings.clone();

    let handler = api::router::<T, P>(api::GothamState {
        inner: ai_gym_state_locked.clone(),
        settings: ai_gym_settings.clone(),
    });

    thread::spawn(move || gotham::start("127.0.0.1:7878", handler));

    if !ai_gym_settings.render_to_buffer {
        return;
    }

    let size = Extent3d {
        width: ai_gym_settings.width,
        height: ai_gym_settings.height,
        ..default()
    };

    for _ in 0..ai_gym_settings.num_agents {
        // This is the texture that will be rendered to.
        let mut render_image = Image {
            texture_descriptor: TextureDescriptor {
                label: None,
                size,
                dimension: TextureDimension::D2,
                format: TextureFormat::Bgra8UnormSrgb,
                mip_level_count: 1,
                sample_count: 1,
                usage: TextureUsages::COPY_SRC
                    | TextureUsages::COPY_DST
                    | TextureUsages::TEXTURE_BINDING
                    | TextureUsages::RENDER_ATTACHMENT,
            },
            ..default()
        };
        render_image.resize(size);
        ai_gym_state
            .render_image_handles
            .push(images.add(render_image));
    }

    let second_pass_layer = RenderLayers::layer(1);

    commands
        .spawn(Camera2dBundle::default())
        .insert(second_pass_layer);

    // Show all camera views in tiled mode
    let window = windows.get_primary_mut().unwrap();
    let number_of_columns = (ai_gym_settings.num_agents as f32).sqrt().ceil() as u32;
    let number_of_rows =
        ((ai_gym_settings.num_agents as f32) / (number_of_columns as f32)).ceil() as u32;
    let mut frames: Vec<Handle<Image>> = Vec::new();
    for f in ai_gym_state.render_image_handles.iter() {
        frames.push(f.clone());
    }
    let offset_x = (size.width * number_of_rows / 2 - size.width / 2) as f32;
    let offset_y = (size.height * number_of_columns / 2 - size.height / 2) as f32;

    for r in 0..number_of_rows {
        for c in 0..number_of_columns {
            let y = (r * size.height) as f32;
            let x = (c * size.width) as f32;

            let i = (c * number_of_columns + r) as usize;
            if i > (frames.len() - 1) {
                continue;
            }

            commands
                .spawn(SpriteBundle {
                    texture: frames[i].clone(),
                    transform: Transform::from_xyz(x - offset_x, y - offset_y, 0.0),
                    ..default()
                })
                .insert(second_pass_layer);
        }
    }

    window.set_resolution(
        (size.width * number_of_rows) as f32,
        (size.height * number_of_columns) as f32,
    );
    window.set_resizable(false);
}

// Pausing the external world each tick
fn control_switch<
    T: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe,
    P: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe + serde::Serialize,
>(
    mut simulation_state: ResMut<State<SimulationState>>,
    time: Res<Time>,
    mut timer: ResMut<SimulationPauseTimer>,
    ai_gym_state: ResMut<state::AIGymState<T, P>>,
    mut pause_event_writer: EventWriter<EventPauseResume>,
) {
    let ai_gym_settings = ai_gym_state.lock().unwrap().settings.clone();
    // This controls control frequency of the environment
    if timer.0.tick(time.delta()).just_finished() {
        // Set current state to control to disable simulation systems
        simulation_state
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
    // mut simulation_state: ResMut<State<SimulationState>>,
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
}
