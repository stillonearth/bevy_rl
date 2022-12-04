# 🏋️‍♀️ bevy_rl

🏗️ Build 🤔 Reinforcement Learning 🏋🏿‍♂️ [Gym](https://gym.openai.com/) environments with 🕊 [Bevy](https://bevyengine.org/) engine to train 👾 AI agents that 💡 can learn from 📺 screen pixels or defined obeservation state.

## Compatibility

| bevy version | bevy_rl version |
| ------------ | :-------------: |
| 0.7          |      0.0.5      |
| 0.8          |      0.8.4      |
| 0.9          |      0.9.4      |

## 📝Features

- Set of APIs to implement OpenAI Gym interface
- REST API to control an agent
- Rendering to RAM membuffer

## 📋 Changelog

- 0.8.4
  - Added object representation of observation space
- 0.9.1
  - Bevy v.0.9 support
  - Minor changes in `Deref` ergonomics
- 0.9.3
  - Fixed a bug when `AIGymState` could not be initialized outside of the crate
- 0.9.4
  - Option to use crate without camera rendering to buffer

## 👩‍💻 Usage

### 1. Define App States

Environment needs to have multiple state, where different system are executed. Typicall you will need to implement InGame, Control and Reset state.

```rust
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
enum AppState {
    InGame,  // where all the game logic is executed
    Control, // A paused state in which bevy_rl waits for agent actions
    Reset,   // A request to reset environment state
}
```

### 2. Define Action Space and Observation Space

Define action and observation spaces. Observation space needs to be Serializable because it's exported via REST API. Action space can be discreet or continuous.

```rust
// Action space
#[derive(Default)]
pub struct Actions {
    ...
}

// Observation space
#[derive(Default, Serialize, Clone)]
pub struct State {
    ...
}
```

### 3. Enable AI Gym Plugin

Width and hight should exceed 256, otherwise wgpu will panic.

```rust
let gym_settings = AIGymSettings {
    width: 256, // set if you need visual observations as state
    height: 256,
    num_agents: 1,
    no_graphics: false, // you can disable rendering to buffer
};

app
    .insert_resource(gym_settings.clone())
    .insert_resource(Arc::new(Mutex::new(AIGymState::<Actions,State>::new(gym_settings.clone()))))
    .add_plugin(AIGymPlugin::<Actions, State>::default())
```

### 3.1 (Optional) Enable Rendering to Buffer

```rust
pub(crate) fn spawn_cameras(
    ai_gym_settings: Res<AIGymSettings>,
    ai_gym_state: Res<AIGymState<Actions, State>>,
) {
    let mut ai_gym_state = ai_gym_state.lock().unwrap();
    for i in 0..ai_gym_settings.num_agents {
        let render_image_handle = ai_gym_state.render_image_handles[i as usize].clone();
        let render_target = RenderTarget::Image(render_image_handle);
        let camera_bundle = Camera3dBundle {
            camera: Camera {
                target: render_target,
                priority: -1,
                ..default()
            },
            ..default()
        };
        commands.spawn(camera_bundle);
    }
}
```

### 4. Implement Environment Logic

`DelayedControlTimer` should pause environment execution to allow agents to take actions.

```rust
#[derive(Resource)]
struct DelayedControlTimer(Timer);
```

Define systems that implement environment logic.

```rust
app.add_startup_system(spawn_cameras);
app.add_system_set(
    SystemSet::on_update(AppState::InGame)
        .with_system(control_switch),
);
app.insert_resource(DelayedControlTimer(Timer::from_seconds(0.1, true))); // 10 Hz
app.add_system_set(
    SystemSet::on_update(AppState::Control)
        // Game Systems
        .with_system(process_control_request) // System that parses user command
        .with_system(process_reset_request),  // System that performs environment state reset
);
app.add_system_set(
    SystemSet::on_enter(AppState::Reset)
        .with_system(reset_envvironment) // System resets environment to initial state
);
```

#### `control_switch` should pause game world and poll `bevy_rl` for agent actions.

```rust
pub(crate) fn control_switch(
    mut app_state: ResMut<State<AppState>>,
    time: Res<Time>,
    mut timer: ResMut<DelayedControlTimer>,
    ai_gym_state: ResMut<AIGymState<Actions, State>>,
    ai_gym_settings: Res<AIGymSettings>,
    mut physics_engine: ResMut<PhysicsEngine>,
) {
  // This controls control frequency of the environment
  if timer.0.tick(time.delta()).just_finished() {

      // Set current state to control to disable simulation systems
      app_state.overwrite_push(AppState::Control).unwrap();

      // Pause time
      physics_engine.pause();
      {
          // ai_gym_state is behind arc mutex, so we need to lock it
          let mut ai_gym_state = ai_gym_state.lock().unwrap();

          // This will tell bevy_rl that environeent is ready to receive actions
          let results = (0..ai_gym_settings.num_agents).map(|_| true).collect();
          ai_gym_state.send_step_result(results);

          // Collect data to build environment state
          // and send it to bevy_rl to be consumable with REST API
          let env_state = State {
              ...
          };
          ai_gym_state.set_env_state(env_state);
      }
  }
}
```

#### `process_reset_request` handles environment reset request.

```rust
pub(crate) fn process_reset_request(
    mut app_state: ResMut<State<AppState>>,
    ai_gym_state: ResMut<AIGymState<Actions, State>>,
) {
    let ai_gym_state = ai_gym_state.lock().unwrap();
    if !ai_gym_state.is_reset_request() {
        return;
    }

    ai_gym_state.receive_reset_request();
    app_state.set(AppState::Reset).unwrap();
}

```

#### `turnbased_text_control_system` parses agent actions and issues commands to agents in environment.

```rust
pub(crate) fn process_control_request(
    ai_gym_state: ResMut<AIGymState<Actions, EnvironmentState>>,
    mut app_state: ResMut<State<AppState>>,
    mut physics_engine: ResMut<PhysicsEngine>,
) {
    let ai_gym_state = ai_gym_state.lock().unwrap();

    // Drop the system if users hasn't sent request this frame
    if !ai_gym_state.is_next_action() {
        return;
    }

    let unparsed_actions = ai_gym_state.receive_action_strings();
    for i in 0..unparsed_actions.len() {
        if let Some(unparsed_action) = unparsed_actions[i].clone() {
            // Parse action and pass it to the game logic
            let action: Vec<...> = serde_json::from_str(&unparsed_action).unwrap();
        }
    }

    physics_engine.resume();
    app_state.pop().unwrap();
}
```

## 💻 AIGymState API

| Method                                             | Description                                |
| -------------------------------------------------- | ------------------------------------------ |
| `send_step_result(results: Vec<bool>) `            | Send upon agents interactions are complete |
| `send_reset_result(result: bool) `                 | Send when reset request is complete        |
| `receive_action_strings(Vec<Option<String>>)`      | Recieve environment for agent actions      |
| `receive_reset_request()`                          | Recieve environment for reset request      |
| `is_next_action() -> bool`                         | Whether agent actions are supplied         |
| `is_reset_request() -> bool`                       | Whether reset request was sent             |
| `set_reward(agent_index: usize, score: f32)`       | Set reward for an agent                    |
| `set_terminated(agent_index: usize, result: bool)` | Set termination status for an agent        |
| `reset()`                                          | Reset bevy_rl state                        |
| `set_env_state(state: State)`                      | Set current environment state              |

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
