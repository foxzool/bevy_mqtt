# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Bevy version

lastest bevy version  is 0.16

## Project Overview

bevy_mqtt is a Rust crate that provides an MQTT client plugin for the Bevy game engine. It integrates MQTT functionality into Bevy's ECS (Entity Component System) architecture, enabling game applications to communicate over MQTT protocol.

## Core Architecture

- **Plugin-based**: Uses Bevy's plugin system (`MqttPlugin`) to integrate MQTT into Bevy apps
- **ECS Integration**: MQTT clients are represented as entities with components (`MqttSetting`, `MqttClient`, `MqttClientConnected`)
- **Event-driven**: Uses Bevy's event system for MQTT events (`MqttEvent`, `MqttConnectError`, `MqttClientError`)
- **Observer Pattern**: Topic subscriptions use Bevy's observer system (`SubscribeTopic` component with observers)
- **Async/Threading**: MQTT connections run in separate threads with channel communication to Bevy's main thread

## Key Components

- `MqttSetting`: Configuration for MQTT connection (broker address, options, channel capacity)
- `MqttClient`: Main MQTT client component wrapping rumqttc::Client
- `SubscribeTopic`: Component for topic subscriptions with regex pattern matching for wildcards
- `TopicMessage`: Event triggered when matching messages arrive

## Development Commands

Build the project:
```
cargo build
```

Run tests:
```
cargo test
```

Run clippy linting:
```
cargo clippy -- -D warnings
```

Format code:
```
cargo fmt --all
```

Run the example (requires MQTT broker):
```
# First start mosquitto broker
docker-compose up mosquitto

# Then run example with websocket feature
cargo run --example pub_and_sub --features websocket
```

## Dependencies

- **Bevy 0.16**: ECS framework components (bevy_app, bevy_ecs, bevy_log, bevy_derive, bevy_reflect)
- **rumqttc 0.24**: MQTT client library
- **flume**: Channel communication between threads
- **regex**: Topic pattern matching for MQTT wildcards

## Testing Setup

- MQTT broker required for integration testing (use docker-compose.yml with mosquitto)
- Example demonstrates both TCP and WebSocket MQTT connections
- Tests include topic pattern matching functionality

## Features

- `websocket`: Enables WebSocket transport for MQTT
- `proxy`: Proxy support
- `native-tls`: Use native TLS implementation
- `rust-tls`: Use Rust TLS implementation

## Version Compatibility

Currently targets Bevy 0.16 - check Cargo.toml for exact version requirements when updating Bevy dependencies.