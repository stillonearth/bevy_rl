# bevy_rl

`bevy_rl` is a tool for building [Reinforcement Learning Gyms](https://gym.openai.com/) 
with [Bevy](https://bevyengine.org/) game engine in Rust.

It lets you to build 3D AI environments to train your AI agents that learn from raw screen pixels.

### Features

* REST API to control an agent
* Rendering to membuffer to get FirstPersonCamera pixels and feed to an agent

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

#### 3. Handling Agent Actions

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
```

#### 4. Handling Agent Actions

```rust
fn turnbased_text_control_system(
    ai_gym_state: ResMut<Arc<Mutex<AIGymState<PlayerActionFlags>>>>,
    mut app_state: ResMut<State<AppState>>,
) {

    // Acquire communication channels
    let step_rx: Receiver<String>;
    let result_tx: Sender<bool>;
    {
        let ai_gym_state = ai_gym_state.lock().unwrap();
        step_rx = ai_gym_state.__step_channel_rx.clone(); // Receiver channel for agent actions
        result_tx = ai_gym_state.__result_channel_tx.clone(); // Sender channel 
    }

    // Return if no input provided from rest API
    if step_rx.is_empty() {
        return;
    }

    let unparsed_action = step_rx.recv().unwrap();
    if unparsed_action == "" {
        // Send negative signal to REST API if action wasn't successful
        result_tx.send(false).unwrap();
        return;
    }

    // Parse an action
    let action = match unparsed_action.as_str() {
        // # PARSE ACTION STRING HERE
    };

    if action.is_none() {
        // Send negative signal to REST API if action wasn't successful
        result_tx.send(false).unwrap(); 
        return;
    }

    // Set reward and whether simulation is terminated
    let score = 1;
    ai_gym_state.rewards.push(score as f32);
    ai_gym_state.is_terminated = false;
    
    // # CONTROL AGENT IN ENVIRONMENT HERE
    
    // Set environment to previous state
    app_state.pop().unwrap();
}
```

#### 5. Handling Environment Reset

```rust
fn execute_reset_request(
    mut app_state: ResMut<State<AppState>>,
    ai_gym_state: ResMut<Arc<Mutex<AIGymState<PlayerActionFlags>>>>,
) {
    let reset_channel_rx: Receiver<bool>;
    {
        let ai_gym_state = ai_gym_state.lock().unwrap();
        reset_channel_rx = ai_gym_state.__reset_channel_rx.clone();
    }

    if reset_channel_rx.is_empty() {
        return;
    }

    reset_channel_rx.recv().unwrap();
    {
        let mut ai_gym_state = ai_gym_state.lock().unwrap();
        ai_gym_state.is_terminated = true;
    }
    
    // # RESET YOUR ENVIRONMENT HERE
}
```

#### 6. Switching Environment to Control State

```rust
fn turnbased_control_system_switch(
    mut app_state: ResMut<State<AppState>>,
    time: Res<Time>,
    mut timer: ResMut<DelayedControlTimer>,
    ai_gym_state: ResMut<Arc<Mutex<AIGymState<PlayerActionFlags>>>>,
) {
    if timer.0.tick(time.delta()).just_finished() {
        app_state.push(AppState::Control).unwrap();

        let ai_gym_state = ai_gym_state.lock().unwrap();

        if ai_gym_state.__result_channel_rx.is_empty() {
            ai_gym_state.__result_channel_tx.send(true).unwrap();
        }
    }
}
```

### Interacting with Environment

**First Person camera pixels**

GET `http://localhost:7878/screen.png`

**Reset Environment**

POST `http://localhost:7878/reset`

**Perform Action**

POST `http://localhost:7878/step` body=ACTION

### Example usage

[BevyStein](https://github.com/stillonearth/BevyStein) is first-person shooter environment made with `bevy_rl`.

### Limitations

`bevy_rl` is early stage of development and has following limitations:

1. Raw pixels are from GPU buffer and do not contain pixels from 2D camera
2. You must be careful with sending signals to step and reset requests or application can deadlock
