// #![feature(associated_type_bounds)]

use std::{marker::PhantomData, thread};

use bevy::{
    prelude::*,
    render::{view::RenderLayers, RenderApp, RenderSet},
};

mod api;
pub mod render;
pub mod state;

use render::copy_from_gpu_to_ram;
pub use state::*;
use wgpu::{Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages};

/// Plugin Settings
#[derive(Clone, Resource, Default)]
pub struct AIGymSettings {
    pub width: u32,
    pub height: u32,
    pub num_agents: u32,
    pub pause_interval: f32,

    // Ignore rending buffer
    pub render_to_buffer: bool,
}

/// This event is fired when user calls `reset` method of the REST API
#[derive(Event)]
pub struct EventReset;

/// This event is fired when user calls `step` method of the REST API
#[derive(Event)]
pub struct EventControl(pub Vec<Option<String>>);

/// This event is fired when an internal timer would need to pause the simulation
#[derive(Event)]
pub struct EventPause;

/// States of the simulation
#[derive(Debug, Clone, Eq, PartialEq, Hash, States, Default, SystemSet)]
pub enum SimulationState {
    Initializing,
    #[default]
    Running,
    PausedForControl,
}

/// Timer to pause the simulation every `AIGymSettings.pause_interval` seconds
#[derive(Resource)]
pub struct SimulationPauseTimer(Timer);

/// bevy_rl plugin
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
        app.add_systems(Startup, setup::<T, P>);

        let ai_gym_state = app
            .world()
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

        // Register events
        app.add_event::<EventReset>();
        app.add_event::<EventControl>();
        app.add_event::<EventPause>();

        // Add system scheduling
        app.insert_state(SimulationState::Initializing)
            .add_systems(Update, control_switch::<T, P>)
            .add_systems(
                Update,
                (
                    process_control_request::<T, P>,
                    process_reset_request::<T, P>,
                )
                    .run_if(in_state(SimulationState::PausedForControl)),
            );

        let render_app = app.get_sub_app_mut(RenderApp).unwrap();

        render_app.add_systems(
            Update,
            copy_from_gpu_to_ram::<T, P>.in_set(RenderSet::Render),
        );
        render_app.insert_resource(ai_gym_state);
    }
}

/// Setup rendering
pub(crate) fn setup<
    T: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe,
    P: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe + serde::Serialize,
>(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    ai_gym_state: ResMut<state::AIGymState<T, P>>,
) {
    let ai_gym_state_locked = ai_gym_state.into_inner().clone();
    let mut ai_gym_state = ai_gym_state_locked.lock().unwrap();
    let ai_gym_settings = ai_gym_state.settings.clone();

    let handler = api::router::<T, P>(api::GothamState {
        inner: ai_gym_state_locked.clone(),
        settings: ai_gym_settings.clone(),
    });

    thread::spawn(move || {
        let result = gotham::start("127.0.0.1:7878", handler);
        if result.is_err() {
            panic!("{:?}", result.err());
        }
    });

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
                view_formats: &[TextureFormat::Bgra8UnormSrgb],
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
        .spawn(Camera2d::default())
        .insert(second_pass_layer.clone());

    // Show all camera views in tiled mode
    // let window = windows.get_primary_mut().unwrap();
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
                .spawn(Sprite {
                    image: frames[i].clone().into(),
                    custom_size: Some(Vec2::new(x - offset_x, y - offset_y)),
                    ..default()
                })
                .insert(second_pass_layer.clone());
        }
    }
}

/// Pausing the external world each tick
fn control_switch<
    T: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe,
    P: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe + serde::Serialize,
>(
    current_simulation_state: Res<State<SimulationState>>,
    mut simulation_state: ResMut<NextState<SimulationState>>,
    time: Res<Time>,
    mut timer: ResMut<SimulationPauseTimer>,
    mut pause_event_writer: EventWriter<EventPause>,
) {
    // let ai_gym_settings = ai_gym_state.lock().unwrap().settings.clone();
    // This controls control frequency of the environment
    if timer.0.tick(time.delta()).just_finished() {
        if *current_simulation_state.get() == SimulationState::Running {
            // Set current state to control to disable simulation systems
            simulation_state.set(SimulationState::PausedForControl);
            // Pause time in all environment
            pause_event_writer.write(EventPause);
            // ai_gym_state is behind arc mutex, so we need to lock it
            // let ai_gym_state = ai_gym_state.lock().unwrap();
            // This will tell bevy_rl that environment is ready to receive actions
            // let results = (0..ai_gym_settings.num_agents).map(|_| true).collect();
            // ai_gym_state.send_step_result(results);
        }
    }
}

/// This is called when user calls reset() in the REST api
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
    reset_event_writer.write(EventReset);
}

/// This is called when user calls step() in the REST api
pub(crate) fn process_control_request<
    T: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe,
    P: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe + serde::Serialize,
>(
    ai_gym_state: ResMut<state::AIGymState<T, P>>,
    mut control_event_writer: EventWriter<EventControl>,
) {
    let ai_gym_state = ai_gym_state.lock().unwrap();
    // Drop the system if users hasn't sent request this frame
    if !ai_gym_state.is_next_action() {
        return;
    }

    let unparsed_actions = ai_gym_state.receive_action_strings();
    control_event_writer.write(EventControl(unparsed_actions));
}
