# AGENTS.md

## Development Commands

### Build Commands
```bash
cargo build                    # Debug build
cargo build --release          # Release build
cargo check                   # Quick check without compilation
```

### Test Commands
```bash
cargo test                    # Run all tests
cargo test --all-features      # Run tests with all features
cargo test --no-default-features --all-features  # Comprehensive test coverage
cargo test --doc              # Run doc tests
cargo test <test_name>        # Run single test (e.g., cargo test test_topic_matches)
```

### Lint & Format Commands
```bash
cargo clippy --all-features -- -D warnings         # Lint with warnings as errors
cargo clippy --examples --all-features -- -D warnings  # Lint examples too
cargo fmt --all                                         # Format code
cargo fmt --all -- --check                           # Check formatting without changes
cargo install --quiet taplo-cli && taplo fmt --check  # Check TOML formatting
```

### Example Commands
```bash
cargo run --example pub_and_sub --features websocket  # Run example with feature
```

## Code Style Guidelines

### Import Organization
- Group imports by crate origin: Bevy → External → std
- One import per line for clarity
- Use nested imports for related items from same crate
- `pub use` for re-exporting external dependencies
- Comment removed unused imports

```rust
use bevy_app::{App, Plugin, Update};
use bevy_ecs::prelude::*;
use bevy_ecs::{event::{EntityEvent, Event}, message::Message};
use bevy_log::{debug, trace};
use bytes::Bytes;
use flume::{Receiver, bounded};
pub use rumqttc;
use std::{collections::VecDeque, thread};
```

### Naming Conventions
- **Types/Components**: PascalCase, use domain-specific prefixes (`MqttClient`, `SubscribeTopic`)
- **Functions/Systems**: snake_case, descriptive verbs (`connect_mqtt_clients`, `handle_mqtt_events`)
- **Variables**: snake_case, concise but meaningful (`entity`, `client`, `event_sender`)
- **Constants/Enums**: PascalCase for variants (`QoS::AtMostOnce`)

### Type Usage
- Use Bevy ECS traits: `Component`, `Event`, `Message`, `Query`, `Commands`
- Implement `Deref`/`DerefMut` for ergonomic access to wrapped types
- Prefer Result<T, E> for fallible operations over unwrap
- Use Bevy's Message system (MessageReader/Writer) for event-driven communication
- Use external types directly when appropriate (rumqttc types, Bytes)

### Error Handling
- Communicate errors via Bevy's Message system, not panics
- Use `Result<T, E>` for operations that can fail
- Log errors with appropriate levels: `debug!` for operational, `error!` for critical
- Use `panic::catch_unwind` for thread safety, convert panics to errors
- Never suppress type errors with `as any` or `@ts-ignore` equivalent

### Logging Levels
- `debug!`: Normal operational flow (connections, subscriptions)
- `trace!`: Detailed execution flow, internal state changes
- Structured logging with context when possible

### Code Organization
- Public types in lib.rs for this single-crate project
- Bevy systems as separate functions, not methods
- Plugin systems registered in `Plugin::build` with `.add_systems()`
- Use Bevy's observer pattern with `.observe()` for reactive behavior
- Tests at bottom of lib.rs with `#[test]` attributes

### Bevy-Specific Patterns
- Component-based architecture for MQTT entities
- Use `Added<T>` and `Without<T>` query filters for lifecycle hooks
- Observer triggers for event handling: `On<Remove, SubscribeTopic>`
- Parent-child relationships with `add_child()` for entity hierarchies
- Event-driven architecture using Message<T> for decoupled systems

### Testing Guidelines
- Unit tests for utility functions (regex matching, cache logic)
- Test names descriptive: `test_topic_regex_escaping`
- Use `unwrap()` in tests only for infallible operations
- Test edge cases: empty collections, boundary conditions

### Performance Considerations
- Use `Local<Vec<T>>` in systems to avoid repeated allocations
- Prefer channel capacity limits (bounded) to prevent memory issues
- Cache regex patterns in components, don't recompile
- Use `VecDeque` for FIFO packet caches
- Clone only when necessary, prefer references in loops
