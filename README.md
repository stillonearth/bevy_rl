# 🏋️‍♀️ bevy_rl

![image](https://github.com/stillonearth/bevy_rl/blob/main/img/dog.gif?raw=true)
![image](https://github.com/stillonearth/bevy_rl/blob/main/img/shooter.gif?raw=true)

##

[![Crates.io](https://img.shields.io/crates/v/bevy_rl.svg)](https://crates.io/crates/bevy_rl)
[![MIT/Apache 2.0](https://img.shields.io/badge/license-MIT%2FApache-blue.svg)](https://github.com/bevyengine/bevy#license)
[![Crates.io](https://img.shields.io/crates/d/bevy_rl.svg)](https://crates.io/crates/bevy_rl)
[![Rust](https://github.com/stillonearth/bevy_rl/workflows/CI/badge.svg)](https://github.com/stillonearth/bevy_rl/actions)

## Reinforcement Learning for Bevy Engine

🏗️ Build 🤔 Reinforcement Learning 🏋🏿‍♂️ [Gym](https://gym.openai.com/) environments with 🕊 [Bevy](https://bevyengine.org/) engine to train 👾 AI agents that 💡 can learn from 📺 screen pixels or defined obeservation state.

## Compatibility

| bevy version | bevy_rl version |
| ------------ | :-------------: |
| 0.7          |      0.0.5      |
| 0.8          |      0.8.4      |
| 0.9          |   0.9.8-beta    |

## 📝Features

- Set of APIs to implement OpenAI Gym interface, such as `reset`, `step`, `render`, `close` and associated simulator states
- Multi-Agent support
- Rendering screen pixels to RAM buffer — for training agents with raw pixels
- REST API to control agents

## 👩‍💻 Usage

### 1. Define Action and Obeservation Space

Observation space needs to be `Serializable` for REST API to work.

```rust
// Action space
#[derive(Default)]
pub struct Actions {
}

// Observation space
#[derive(Default, Serialize, Clone)]
pub struct EnvironmentState {
}
```

### 2. Enable AI Gym Plugin

Width and height should exceed 256, otherwise wgpu will panic.

```rust
// Setup bevy_rl
let ai_gym_state = AIGymState::<Actions, State>::new(AIGymSettings {
    width: u32,              // Width and height of the screen
    height: u32,             // ...
    num_agents: 1,           // Number of agents — each will get a camera handle
    render_to_buffer: false, // You can disable rendering to buffer
    pause_interval: 0.01,    // 100 Hz
    ..default()
});
app.insert_resource(ai_gym_state)
    .add_plugin(AIGymPlugin::<Actions, EnvironmentState>::default());
```

### 2.1 (Optional) Enable Rendering to Buffer

If your environment exports raw pixels, you will need to attach a render target to each camera you want to export pixels from.

```rust
pub(crate) fn spawn_cameras(
    ai_gym_state: Res<AIGymState<Actions, EnvironmentState>>,
) {
    let mut ai_gym_state = ai_gym_state.lock().unwrap();
    let ai_gym_settings = ai_gym_state.settings.clone();

    for i in 0..ai_gym_settings.num_agents {
        let render_image_handle = ai_gym_state.render_image_handles[i as usize].clone();
        let render_target = RenderTarget::Image(render_image_handle);
        let camera_bundle = Camera3dBundle {
            camera: Camera {
                target: render_target,  // Render target is baked in bevy_rl and used to export pixels
                priority: -1,           // set to -1 to render at the firstmost pass
                ..default()
            },
            ..default()
        };
        commands.spawn(camera_bundle);
    }
}
```

### 4. Handle bevy_rl events

`bevy_rl` will communicate with your environment through events. You can use `EventReader` to read events and respond to them. Those event are from REST API or from a timer that pauses the simulation with given interval (`AIGymSettings.pause_interval`).

| Event          | Description                        | Usage                                                                                      |
| -------------- | ---------------------------------- | ------------------------------------------------------------------------------------------ |
| `EventReset`   | Reset environment to initial state | You should rebuild your evnironment here                                                   |
| `EventControl` | Switch to control state            | You should recieve actions here and apply them to your environment (and resume simulation) |
| `EventPause`   | Pause environment execution        | Pause physics engine or game clock and take snapshot of your game state                    |

Here's example of how to handle those events:

```rust
// EventPauseResume
fn bevy_rl_pause_request(
    mut pause_event_reader: EventReader<EventPauseResume>,
    ai_gym_state: Res<AIGymState<Actions, State>>,
) {
    for _ in pause_event_reader.iter() {
        // Pause simulation (physics engine)
        // ...
        // Collect state into serializable struct
        let env_state = EnvironmentState(...);
        // Set bevy_rl gym state
        let mut ai_gym_state = ai_gym_state.lock().unwrap();
        ai_gym_state.set_env_state(env_state);
    }
}

// EventControl
fn bevy_rl_control_request(
    mut pause_event_reader: EventReader<EventControl>,
    mut simulation_state: ResMut<State<SimulationState>>,
) {
    for control in pause_event_reader.iter() {
        let unparsed_actions = &control.0;
        for i in 0..unparsed_actions.len() {
            if let Some(unparsed_action) = unparsed_actions[i].clone() {
                let action: Vec<f64> = serde_json::from_str(&unparsed_action).unwrap();
                // Pass control inputs to your agents
                // ...
            }
        }
        // Resume simulation (physics engine)
        // ...
        // Return to running state; note that it uses pop/push to avoid
        // entering `SystemSet::on_enter(SimulationState::Running)` which initialized game world anew
        simulation_state.pop().unwrap();
    }
}

/// Handle bevy_rl::EventReset
pub(crate) fn bevy_rl_reset_request(
    mut reset_event_reader: EventReader<EventReset>,
    mut commands: Commands,
    mut walls: Query<Entity, &Wall>,
    mut players: Query<(Entity, &Actor)>,
    mut simulation_state: ResMut<State<SimulationState>>,
    ai_gym_state: Res<AIGymState<Actions, EnvironmentState>>,
) {
    if reset_event_reader.iter().count() == 0 {
        return;
    }

    // Reset envrionment state here

    // Return simulation in Running state
    simulation_state.set(SimulationState::Running).unwrap();

    // tell bevy_rl that environment is reset and return response to REST API
    let ai_gym_state = ai_gym_state.lock().unwrap();
    ai_gym_state.send_reset_result(true);
}
```

Register systems to handle bevy_rl events.

```rust
app.add_system_set(
    SystemSet::on_update(SimulationState::PausedForControl)
        .with_system(bevy_rl_control_request)
        .with_system(bevy_rl_reset_request)
        .with_system(bevy_rl_pause_request),
);
```

## 💻 AIGymState API

Those methods are available on `AIGymState` resource. You should use them to alter bevy_rl internal state.

| Method                                             | Description                         | Usage                                                                                        |
| -------------------------------------------------- | ----------------------------------- | -------------------------------------------------------------------------------------------- |
| `set_reward(agent_index: usize, score: f32)`       | Set reward for an agent             | When a certain event happens, you can set reward for an agent.                               |
| `set_terminated(agent_index: usize, result: bool)` | Set termination status for an agent | Once your agent is killed, you should set it's status to `true`. Useful for Multi-agent.     |
| `reset()`                                          | Reset bevy_rl state                 | You should call this method when you reset your environment to clear exported state history  |
| `set_env_state(state: State)`                      | Set current environment state       | When you serialize your environment state, you should set it here.                           |
| `send_reset_result(result: bool)`                  | Send reset result to REST API       | You should call this method when you have reset your environment to sychronize with REST API |

## 🌐 REST API

Accessing `bevy_rl`-enabled environment is possible through REST API. You can use any HTTP client to communicate with it. Here's a list of available endpoints:

| Method            | Verb     | bevy_rl version                               |
| ----------------- | -------- | --------------------------------------------- |
| Camera Pixels     | **GET**  | `http://localhost:7878/visual_observations`   |
| State             | **GET**  | `http://localhost:7878/state`                 |
| Reset Environment | **POST** | `http://localhost:7878/reset`                 |
| Step              | **GET**  | `http://localhost:7878/step` `payload=ACTION` |

One would wrap those endpoints into a client library (python) to make it easier to use.

## ✍️ Examples

- [bevy_rl_shooter](https://github.com/stillonearth/bevy_rl_shooter) — example FPS project
- [bevy_quadruped_neural_control](https://github.com/stillonearth/bevy_quadruped_neural_control) — quadruped locomotion with bevy_mujoco and bevy_rl
