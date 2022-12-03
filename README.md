# 🏋️‍♀️ bevy_rl

🏗️ Build 🤔 Reinforcement Learning 🏋🏿‍♂️ [Gym](https://gym.openai.com/) environments with 🕊 [Bevy](https://bevyengine.org/) engine to train 👾 AI agents that 💡 learn from 📺 screen pixels.

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

```rust

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
enum AppState {
    InGame,  // where all the game logic is executed
    Control, // A paused state in which bevy_rl waits for agent actions
    Reset,   // A request to reset environment state
}
```

### 2. Define Action Space and Observation Space

A action space is a set of actions that an agent can take. An observation space is a set of observations that an agent can see. Action space can be discrete or continuous. Observations should be serializable to JSON with `serde_json` crate.

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
    width: 256,
    height: 256,
    num_agents: 16,
    no_graphics: false,
};

app
    .insert_resource(gym_settings.clone())
    .insert_resource(Arc::new(Mutex::new(AIGymState::<Actions,State>::new(gym_settings.clone()))))
    .add_plugin(AIGymPlugin::<Actions, State>::default())
```

### 4. Implement Environment Logic

`DelayedControlTimer` should pause environment execution to allow agents to take actions.

```rust
struct DelayedControlTimer(Timer);
```

Define systems that implement environment logic.

```rust
app.add_system_set(
    SystemSet::on_update(AppState::InGame)
        .with_system(turnbased_control_system_switch),
);

app.insert_resource(DelayedControlTimer(Timer::from_seconds(0.1, true))); // 10 Hz
app.add_system_set(
    SystemSet::on_update(AppState::Control)
        // Game Systems
        .with_system(turnbased_text_control_system) // System that parses user command
        .with_system(execute_reset_request),        // System that performs environment state reset
);
```

- `turnbased_control_system_switch` should pause game world and poll `bevy_rl` for agent actions.
- `execute_reset_request` handles environment reset request.
- `turnbased_text_control_system` parses agent actions and issues commands to agents in environment.

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
| `set_env_state(state: B)`                          | Set current environment state              |

## 🌐 REST API

| Method            | Verb     | bevy_rl version                               |
| ----------------- | -------- | --------------------------------------------- |
| Camera Pixels     | **GET**  | `http://localhost:7878/visual_observations`   |
| State             | **GET**  | `http://localhost:7878/state`                 |
| Reset Environment | **POST** | `http://localhost:7878/reset`                 |
| Step              | **GET**  | `http://localhost:7878/step` `payload=ACTION` |

## ✍️ Examples

[bevy_rl_shooter](https://github.com/stillonearth/bevy_rl_shooter) — example FPS project
