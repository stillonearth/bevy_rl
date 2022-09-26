# bevy_rl

<img width="209" alt="image" src="https://user-images.githubusercontent.com/97428129/168558015-e6ddd435-dfdf-4f03-b352-070074f5a392.png">

Build [Reinforcement Learning Gym](https://gym.openai.com/) environments
with [Bevy](https://bevyengine.org/) engine to train AI agents that learn from raw screen pixels.

### Compatibility

| bevy version | bevy_rl version |
| ------------ | :-------------: |
| 0.7          |      0.0.5      |
| 0.8          |      0.8.2      |

### Features

- Set of APIs to implement OpenAI Gym interface
- REST API to control an agent
- Rendering to RAM membuffer

### Usage

#### 1. Define Action Space and Application State

```rust

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
enum AppState {
    InGame,  // Actve state
    Control, // A paused state in which application waits for agent input
    Reset,   // A request to reset environment state
}

// List of possible agent actions (discrete variant)
bitflags! {
    #[derive(Default)]
    pub struct PlayerActionFlags: u32 {
        const IDLE = 1 << 0;
        const FORWARD = 1 << 1;
        const BACKWARD = 1 << 2;
        const LEFT = 1 << 3;
        const RIGHT = 1 << 4;
        const TURN_LEFT = 1 << 5;
        const TURN_RIGHT = 1 << 6;
        const SHOOT = 1 << 7;
    }
}
```

#### 2. Enable AI Gym Plugin

```rust
    let gym_settings = AIGymSettings {
        width: 256,
        height: 256,
        num_agents: 2,
    };

    app
        // bevy_rl initialization
        .insert_resource(gym_settings.clone())
        .insert_resource(Arc::new(Mutex::new(AIGymState::<PlayerActionFlags>::new(
```

#### 3. Make sure environment is controllable at discreet time steps

```rust
struct DelayedControlTimer(Timer);
```

```rust
app.insert_resource(DelayedControlTimer(Timer::from_seconds(0.1, true))); // 10 Hz
app.add_system_set(
    SystemSet::on_update(AppState::Control)
        // Game Systems
        .with_system(turnbased_text_control_system) // System that parses user command
        .with_system(execute_reset_request),        // System that performs environment state reset
);


app.add_system_set(
    SystemSet::on_update(AppState::InGame)
        .with_system(turnbased_control_system_switch),
);

```

```rust
fn turnbased_control_system_switch(
    mut app_state: ResMut<State<AppState>>,
    time: Res<Time>,
    mut timer: ResMut<DelayedControlTimer>,
    ai_gym_state: ResMut<Arc<Mutex<AIGymState<PlayerActionFlags>>>>,
) {
    if timer.0.tick(time.delta()).just_finished() {
        app_state.push(AppState::Control);
        physics_time.pause();

        let ai_gym_state = ai_gym_state.lock().unwrap();
        ai_gym_state.send_step_result(true);
    }
}
```

#### 4. Handle Reset & Agent Actions from REST API in Bevy Environment

```rust
pub(crate) fn execute_reset_request(
    mut app_state: ResMut<State<AppState>>,
    ai_gym_state: ResMut<Arc<Mutex<AIGymState<PlayerActionFlags>>>>,
) {
    let ai_gym_state = ai_gym_state.lock().unwrap();
    if !ai_gym_state.is_reset_request() {
        return;
    }

    ai_gym_state.receive_reset_request();
    app_state.set(AppState::Reset).unwrap();
}

pub(crate) fn turnbased_control_system_switch(
    mut app_state: ResMut<State<AppState>>,
    time: Res<Time>,
    mut timer: ResMut<DelayedControlTimer>,
    ai_gym_state: ResMut<Arc<Mutex<AIGymState<PlayerActionFlags>>>>,
    ai_gym_settings: Res<AIGymSettings>,
    mut physics_time: ResMut<PhysicsTime>,
) {
    if timer.0.tick(time.delta()).just_finished() {
        app_state.overwrite_push(AppState::Control).unwrap();
        physics_time.pause();

        let ai_gym_state = ai_gym_state.lock().unwrap();
        let results = (0..ai_gym_settings.num_agents).map(|_| true).collect();
        ai_gym_state.send_step_result(results);
    }
}

pub(crate) fn turnbased_text_control_system(
    agent_movement_q: Query<(&mut heron::prelude::Velocity, &mut Transform, &Actor)>,
    collision_events: EventReader<CollisionEvent>,
    event_gun_shot: EventWriter<EventGunShot>,
    ai_gym_state: ResMut<Arc<Mutex<AIGymState<PlayerActionFlags>>>>,
    ai_gym_settings: Res<AIGymSettings>,
    mut app_state: ResMut<State<AppState>>,
    mut physics_time: ResMut<PhysicsTime>,
) {
    let mut ai_gym_state = ai_gym_state.lock().unwrap();

    if !ai_gym_state.is_next_action() {
        return;
    }

    let unparsed_actions = ai_gym_state.receive_action_strings();
    let mut actions: Vec<Option<PlayerActionFlags>> =
        (0..ai_gym_settings.num_agents).map(|_| None).collect();

    for i in 0..unparsed_actions.len() {
        let unparsed_action = unparsed_actions[i].clone();
        ai_gym_state.set_reward(i, 0.0);

        if unparsed_action.is_none() {
            actions[i] = None;
            continue;
        }

        let action = match unparsed_action.unwrap().as_str() {
            "FORWARD" => Some(PlayerActionFlags::FORWARD),
            "BACKWARD" => Some(PlayerActionFlags::BACKWARD),
            "LEFT" => Some(PlayerActionFlags::LEFT),
            "RIGHT" => Some(PlayerActionFlags::RIGHT),
            "TURN_LEFT" => Some(PlayerActionFlags::TURN_LEFT),
            "TURN_RIGHT" => Some(PlayerActionFlags::TURN_RIGHT),
            "SHOOT" => Some(PlayerActionFlags::SHOOT),
            _ => None,
        };

        actions[i] = action;
    }

    physics_time.resume();
    control_agents(actions, agent_movement_q, collision_events, event_gun_shot);

    app_state.pop().unwrap();
}
```

### REST API

| Method            | Verb     | bevy_rl version                            |
| ----------------- | -------- | ------------------------------------------ |
| Camera Pixels     | **GET**  | `http://localhost:7878/screen.png`         |
| Reset Environment | **POST** | `http://localhost:7878/reset`              |
| Step              | **POST** | `http://localhost:7878/step` `body=ACTION` |

### Examples

[bevy_rl_shooter](https://github.com/stillonearth/bevy_rl_shooter) — example FPS project
