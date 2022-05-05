# bevy_rl

`bevy_rl` lets you build [Reinforcement Learning Gyms](https://gym.openai.com/) environments
with [Bevy](https://bevyengine.org/) game engine in Rust language to train AI agents that learn from raw screen pixels.

### Features

* Set of APIs to implement OpenAI Gym interface
* REST API to control an agent
* Rendering to RAM membuffer

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
app
    // Plugin settings
    .insert_resource(AIGymSettings { \\ viewport settings
        width: 768,  
        height: 768,
    })
    
    // Actions
    .insert_resource(Arc::new(Mutex::new(AIGymState::<PlayerActionFlags> { 
        ..Default::default()
    })))

    // Plugin
    .add_plugin(AIGymPlugin::<PlayerActionFlags>::default());
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

#### 4. Handle Agent Actions from REST API in Bevy Environment

```rust
fn turnbased_text_control_system(
    ai_gym_state: ResMut<Arc<Mutex<AIGymState<PlayerActionFlags>>>>,
    mut app_state: ResMut<State<AppState>>,
) {
    let mut ai_gym_state = ai_gym_state.lock().unwrap();

    if !ai_gym_state.is_next_action() {
        return;
    }

    let unparsed_action = ai_gym_state.receive_action_string();

    if unparsed_action == "" {
        ai_gym_state.send_step_result(false);
        return;
    }

    let action = match unparsed_action.as_str() {
        "FORWARD" => Some(PlayerActionFlags::FORWARD),
        "BACKWARD" => Some(PlayerActionFlags::BACKWARD),
        "LEFT" => Some(PlayerActionFlags::LEFT),
        "RIGHT" => Some(PlayerActionFlags::RIGHT),
        "TURN_LEFT" => Some(PlayerActionFlags::TURN_LEFT),
        "TURN_RIGHT" => Some(PlayerActionFlags::TURN_RIGHT),
        "SHOOT" => Some(PlayerActionFlags::SHOOT),
        _ => None,
    };

    if action.is_none() {
        ai_gym_state.send_step_result(false);
        return;
    }

    let player = player_query.iter().find(|e| e.name == "Player 1").unwrap();
    {
        ai_gym_state.set_score(player.score as f32);
    }

    physics_time.resume();

    control_player(
        action.unwrap(),
        player_movement_q,
        collision_events,
        event_gun_shot,
    );

    app_state.pop().unwrap();
}
```

#### 5. Handle Environment Reset Requests from REST API in Bevy Environment

```rust
fn execute_reset_request(
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
```


### Interacting with Environment

#### Camera Pixels

**GET** `http://localhost:7878/screen.png`

#### Reset Environment

**POST** `http://localhost:7878/reset`

#### Perform Action

**POST** `http://localhost:7878/step` `body=ACTION`

### Example usage

[BevyStein](https://github.com/stillonearth/BevyStein) is first-person shooter environment made with `bevy_rl`.

### Limitations

1. Raw pixels are from GPU buffer 3D camera do not contain pixels from 2D camera
