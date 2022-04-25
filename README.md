# bevy_rl

`bevy_rl` is a tool for building [Reinforcement Learning Gyms](https://gym.openai.com/) 
with [Bevy](https://bevyengine.org/) game engine in Rust.

It lets you to build 3D AI environments to train your AI agents that learn from raw screen pixels.

### Features

* REST API to control an agent
* Rendering to membuffer to get FirstPersonCamera pixels and feed to an agent

### Usage

1. Define Action Spac eand Application State

```rust
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
enum AppState {
    InGame,
    Control,
    Reset,
}

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

2. Enable AI Gym across your application

```rust
    app
        .insert_resource(AIGymSettings { \\ viewport settings
            width: 768,  
            height: 768,
        })
        .insert_resource(Arc::new(Mutex::new(AIGymState::<PlayerActionFlags> { \\ user-defined action space
            ..Default::default()
        })));
```

3. Define a application state which awaits for agent action

```rust
struct DelayedControlTimer(Timer); \\ Descreet timesteps in which agent input is expected
```

```rust
 
        app.insert_resource(DelayedControlTimer(Timer::from_seconds(0.1, true))); \\ 10 Hz

        app.add_system_set(
            SystemSet::on_update(AppState::Control)
                // Game Systems
                .with_system(turnbased_text_control_system) \\ System that parses user command
                .with_system(execute_reset_request),        \\ System that performs environment state reset
        );
    }
```

4. Handling agent actions

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

    // Calculate current step's environment rewards
    let score = 1;
    ai_gym_state.rewards.push(score as f32);
    ai_gym_state.is_terminated = false;
    

    // Resume time in environment
    // 
    // # CONTROL AGENT IN ENVIRONMENT HERE
    //
    // Put environment in previous state

    app_state.pop().unwrap();
}
```

5. Handling environment reset requests

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

6. Switch system to control state

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

GET http://localhost:7878/screen.png

**Reset Environment**

POST http://localhost:7878/reset

**Perform Action**

POST http://localhost:7878/step TURN_LEFT

Reset and Step handles would return current state, is_terminated flag and reward as json

### Example usage

[BevyStein](https://github.com/stillonearth/BevyStein) is first-person shooter environment made with `bevy_rl`.

### Limitations

`bevy_rl` is in early stage of development and has following limitations:

1. Raw pixels are from GPU buffer and do not contain pixels from 2D camera
2. You must be careful with sending signals to step and reset requests
3. Set `is_terminated` and `reward` in your `turnbased_control_system`.
