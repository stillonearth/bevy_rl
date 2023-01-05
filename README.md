# 🏋️‍♀️ bevy_rl

![image](https://github.com/stillonearth/bevy_rl/blob/main/img/dog.gif?raw=true)
![image](https://github.com/stillonearth/bevy_rl/blob/main/img/shooter.gif?raw=true)

🏗️ Build 🤔 Reinforcement Learning 🏋🏿‍♂️ [Gym](https://gym.openai.com/) environments with 🕊 [Bevy](https://bevyengine.org/) engine to train 👾 AI agents that 💡 can learn from 📺 screen pixels or defined obeservation state.

## Compatibility

| bevy version | bevy_rl version |
| ------------ | :-------------: |
| 0.7          |      0.0.5      |
| 0.8          |      0.8.4      |
| 0.9          |   0.9.8-beta    |

## 📝Features

- Set of APIs to implement OpenAI Gym interface
- REST API to control an agent
- Rendering to RAM membuffer

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
pub struct State {
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
    .add_plugin(AIGymPlugin::<Actions, State>::default());
```

### 2.1 (Optional) Enable Rendering to Buffer

If your environment exports raw pixels, you will need to attach a render target to each camera of your agents.

```rust
pub(crate) fn spawn_cameras(
    ai_gym_state: Res<AIGymState<Actions, State>>,
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

| Event              | Description                           |
| ------------------ | ------------------------------------- |
| `EventReset`       | Reset environment to initial state    |
| `EventControl`     | Switch to control state               |
| `EventPauseResume` | Pause or resume environment execution |

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
        // Switch to SimulationState::Running state of bevy_rl
        simulation_state.set(SimulationState::Running);
    }
}
```

Register systems to handle bevy_rl events.

```rust
// bevy_rl events
app.add_system(bevy_rl_pause_request);
app.add_system(bevy_rl_control_request);
```

## 💻 AIGymState API

| Method                                             | Description                         |
| -------------------------------------------------- | ----------------------------------- |
| `set_reward(agent_index: usize, score: f32)`       | Set reward for an agent             |
| `set_terminated(agent_index: usize, result: bool)` | Set termination status for an agent |
| `reset()`                                          | Reset bevy_rl state                 |
| `set_env_state(state: State)`                      | Set current environment state       |

## 🌐 REST API

| Method            | Verb     | bevy_rl version                               |
| ----------------- | -------- | --------------------------------------------- |
| Camera Pixels     | **GET**  | `http://localhost:7878/visual_observations`   |
| State             | **GET**  | `http://localhost:7878/state`                 |
| Reset Environment | **POST** | `http://localhost:7878/reset`                 |
| Step              | **GET**  | `http://localhost:7878/step` `payload=ACTION` |

## ✍️ Examples

- [bevy_rl_shooter](https://github.com/stillonearth/bevy_rl_shooter) — example FPS project
- [bevy_quadruped_neural_control](https://github.com/stillonearth/bevy_quadruped_neural_control) — quadruped locomotion with bevy_mujoco and bevy_rl
