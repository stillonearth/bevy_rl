use bevy::prelude::*;
use bevy_rl::*;
use serde::Serialize;

#[derive(Default, Clone, Serialize, Debug)]
pub struct Agent {
    location: (f32, f32),
    health: f32,
}

#[derive(Default, Deref, DerefMut, Clone)]
pub struct Actions(String);

// Observation space
#[derive(Default, Deref, DerefMut, Clone, Serialize, Resource)]
pub struct EnvironmentState {
    pub agents: Vec<Agent>,
}

fn bevy_rl_pause_request(
    mut pause_event_reader: EventReader<EventPause>,
    ai_gym_state: Res<AIGymState<Actions, EnvironmentState>>,
    env_state: Res<EnvironmentState>,
) {
    for _ in pause_event_reader.iter() {
        let mut ai_gym_state = ai_gym_state.lock().unwrap();
        ai_gym_state.set_env_state(env_state.clone());
    }
}

#[allow(unused_must_use)]
#[allow(clippy::needless_range_loop)]
fn bevy_rl_control_request(
    mut pause_event_reader: EventReader<EventControl>,
    mut simulation_state: ResMut<NextState<SimulationState>>,
    mut env_state: ResMut<EnvironmentState>,
) {
    for control in pause_event_reader.iter() {
        let unparsed_actions = &control.0;
        for i in 0..unparsed_actions.len() {
            if let Some(unparsed_action) = unparsed_actions[i].clone() {
                match unparsed_action.as_str() {
                    "DOWN" => env_state.agents[i].location.1 -= 1.0,
                    "UP" => env_state.agents[i].location.1 += 1.0,
                    "LEFT" => env_state.agents[i].location.0 -= 1.0,
                    "RIGHT" => env_state.agents[i].location.0 += 1.0,
                    _ => {}
                }
            }
        }

        simulation_state.set(SimulationState::Running);
    }
}

fn start_bevy_app() {
    let num_agents = 5;
    let initial_state = EnvironmentState {
        agents: vec![Agent::default(); num_agents],
    };

    let mut app = App::new();

    // Basic bevy setup
    app.add_plugins(MinimalPlugins);
    app.add_plugin(WindowPlugin::default());
    app.add_plugin(AssetPlugin::default());
    app.add_plugin(ImagePlugin::default());

    // Setup bevy_rl
    let ai_gym_state = AIGymState::<Actions, EnvironmentState>::new(AIGymSettings {
        num_agents: num_agents as u32,
        render_to_buffer: false,
        pause_interval: 0.0001,
        ..default()
    });
    app.insert_resource(ai_gym_state)
        .add_plugin(AIGymPlugin::<Actions, EnvironmentState>::default());

    // initialize app state
    app.insert_resource(initial_state);

    // bevy_rl events
    app.add_system(bevy_rl_pause_request);
    app.add_system(bevy_rl_control_request);

    // Run for 1M frames
    for _ in 0..1000000 {
        // sleep for 1/60 of a second
        std::thread::sleep(std::time::Duration::from_millis(16));
        app.update();
    }
}

#[test]
/// This test would start a basic bevy_rl app and test the 3 scenarios:
/// 1. Test `state` endpoint with environment original state
/// 2. Test `step` endpoint with 5 actions for each agent
/// 3. Test `state` endpoint with environment state after actions taken to make sure
/// it matches the expected state
fn test_api_state_step() {
    // Start bevy app in a separate thread
    std::thread::spawn(|| {
        start_bevy_app();
    });

    // let bevy app start REST API
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Test `state` endpoint
    let response = reqwest::blocking::get("http://localhost:7878/state")
        .unwrap()
        .text()
        .unwrap();

    let expected_response = r#"{"agents":[{"health":0.0,"location":[0.0,0.0]},{"health":0.0,"location":[0.0,0.0]},{"health":0.0,"location":[0.0,0.0]},{"health":0.0,"location":[0.0,0.0]},{"health":0.0,"location":[0.0,0.0]}]}"#;
    assert_eq!(response, expected_response);

    // Test `step` endpoint
    #[derive(Serialize)]
    struct RESTAPIAction {
        action: String,
    }

    // bevy_rl expects each action to be in format: {"action": string:serialized_action}
    // bevy_rl will deserialize it's internal AgentAction and your environment will need to
    // deserialize the action string to the correct type

    let actions: [RESTAPIAction; 5] = [
        RESTAPIAction {
            action: "DOWN".to_string(),
        },
        RESTAPIAction {
            action: "UP".to_string(),
        },
        RESTAPIAction {
            action: "LEFT".to_string(),
        },
        RESTAPIAction {
            action: "RIGHT".to_string(),
        },
        RESTAPIAction {
            action: "IDLE".to_string(),
        },
    ];

    let actions_json = serde_json::to_string(&actions).unwrap();
    let response =
        reqwest::blocking::get(format!("http://localhost:7878/step?payload={actions_json}"))
            .unwrap()
            .text()
            .unwrap();

    std::thread::sleep(std::time::Duration::from_millis(1000));

    let expected_response = r#"[{"is_terminated":false,"reward":0.0},{"is_terminated":false,"reward":0.0},{"is_terminated":false,"reward":0.0},{"is_terminated":false,"reward":0.0},{"is_terminated":false,"reward":0.0}]"#;
    assert!(response == expected_response);

    let response = reqwest::blocking::get("http://localhost:7878/state")
        .unwrap()
        .text()
        .unwrap();

    let expected_response = r#"{"agents":[{"health":0.0,"location":[0.0,-1.0]},{"health":0.0,"location":[0.0,1.0]},{"health":0.0,"location":[-1.0,0.0]},{"health":0.0,"location":[1.0,0.0]},{"health":0.0,"location":[0.0,0.0]}]}"#;

    assert!(response == expected_response);
}
