# bevy_mqtt

[![crates.io](https://img.shields.io/crates/v/bevy_mqtt)](https://crates.io/crates/bevy_mqtt)
[![MIT/Apache 2.0](https://img.shields.io/badge/license-MIT%2FApache-blue.svg)](https://github.com/Seldom-SE/seldom_pixel#license)
[![crates.io](https://img.shields.io/crates/d/bevy_mqtt)](https://crates.io/crates/bevy_mqtt)
[![CI](https://github.com/foxzool/bevy_mqtt/workflows/CI/badge.svg)](https://github.com/foxzool/bevy_mqtt/actions)
[![Documentation](https://docs.rs/bevy_mqtt/badge.svg)](https://docs.rs/bevy_mqtt)

A robust, secure MQTT client plugin for the Bevy game engine with comprehensive error handling and performance
optimizations.

## Features

- 🔌 **Easy Integration** - Simple plugin architecture that fits naturally into Bevy's ECS
- 🔒 **Security First** - Regex injection protection and robust error handling
- ⚡ **High Performance** - Optimized message dispatch with memory reuse patterns
- 🌐 **Multiple Transports** - Support for TCP and WebSocket connections
- 📦 **Message Caching** - Built-in packet caching with configurable capacity limits
- 🎯 **Topic Matching** - MQTT wildcard support with secure regex pattern matching
- 🔄 **Auto Reconnection** - Automatic reconnection handling on connection failures
- 📊 **Event-Driven** - Full integration with Bevy's event system

## Quick Start

First, run an MQTT broker like [Mosquitto](https://mosquitto.org/):

```bash
# Using Docker
docker run -it -p 1883:1883 -p 9001:9001 eclipse-mosquitto:2

# Or using docker-compose (see docker-compose.yml in this repo)
docker-compose up
```

Then add bevy_mqtt to your `Cargo.toml`:

```toml
[dependencies]
bevy_mqtt = "0.8.0"
```

## Basic Example

```rust
use bevy::{prelude::*, time::common_conditions::on_timer};
use bevy_mqtt::{
    MqttClient, MqttClientConnected, MqttClientError, MqttConnectError, MqttEvent,
    MqttPlugin, MqttPublishOutgoing, MqttSetting, SubscribeTopic, TopicMessage,
};
use bevy_ecs::{message::MessageReader, message::MessageWriter};
use rumqttc::{MqttOptions, QoS};
use std::time::Duration;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, MqttPlugin))
        .add_systems(Startup, setup_mqtt_client)
        .add_systems(Update, (
            subscribe_to_topics,
            handle_messages,
            handle_errors,
            publish_messages.run_if(on_timer(Duration::from_secs(5))),
        ))
        .run();
}

fn setup_mqtt_client(mut commands: Commands) {
    // TCP connection
    commands.spawn(MqttSetting {
        mqtt_options: MqttOptions::new("bevy-game-client", "localhost", 1883),
        cap: 100, // Channel capacity
    });
}

fn subscribe_to_topics(
    mut commands: Commands,
    mqtt_clients: Query<Entity, Added<MqttClientConnected>>,
) {
    for client_entity in mqtt_clients.iter() {
        // Subscribe using component-based approach with MQTT wildcards
        let topic_entity = commands
            .spawn(SubscribeTopic::new("game/+/events", QoS::AtMostOnce).unwrap())
            .observe(|trigger: Trigger<TopicMessage>| {
                println!("Game event: {}", trigger.event().topic);
            })
            .id();

        // Link topic subscription to client
        commands.entity(client_entity).add_child(topic_entity);
    }
}

fn handle_messages(mut mqtt_events: MessageReader<MqttEvent>) {
    for event in mqtt_events.read() {
        if let rumqttc::Event::Incoming(rumqttc::Incoming::Publish(publish)) = &event.event {
            println!("Received on {}: {:?}", publish.topic, publish.payload);
        }
    }
}

fn handle_errors(
    mut connect_errors: MessageReader<MqttConnectError>,
    mut client_errors: MessageReader<MqttClientError>,
) {
    for error in connect_errors.read() {
        eprintln!("MQTT connection error: {:?}", error.error);
    }

    for error in client_errors.read() {
        eprintln!("MQTT client error: {:?}", error.error);
    }
}

// Event-driven publishing (recommended)
fn publish_messages(
    mqtt_clients: Query<Entity, With<MqttClientConnected>>,
    mut publish_events: MessageWriter<MqttPublishOutgoing>,
) {
    for client_entity in mqtt_clients.iter() {
        publish_events.write(MqttPublishOutgoing {
            entity: client_entity,
            topic: "game/player/position".to_string(),
            qos: QoS::AtLeastOnce,
            retain: false,
            payload: b"x:100,y:200".to_vec(),
        });
    }
}
```

## Advanced Features

### Message Caching

```rust
use bevy_mqtt::PacketCache;

// Add message caching to topic subscriptions
commands
.spawn((
SubscribeTopic::new("game/chat", QoS::AtMostOnce).unwrap(),
PacketCache::new(50), // Keep last 50 messages
))
.observe( | trigger: Trigger<TopicMessage>| {
println ! ("Chat message: {:?}", trigger.event().payload);
});
```

### WebSocket Support

```rust
use rumqttc::Transport;

let mut mqtt_options = MqttOptions::new("websocket-client", "ws://localhost:9001", 9001);
mqtt_options.set_transport(Transport::Ws);

commands.spawn(MqttSetting {
mqtt_options,
cap: 100,
});
```

### Secure Topic Patterns

The library automatically escapes regex metacharacters in topic patterns while preserving MQTT wildcards:

```rust
// Safe - regex metacharacters are escaped, MQTT wildcards preserved
SubscribeTopic::new("sensor/data[temp]/+", QoS::AtMostOnce).unwrap();

// This matches: "sensor/data[temp]/kitchen" but not "sensor/dataXtemp]/kitchen"
```

## Supported Versions

| bevy | bevy_mqtt     |
|------|---------------|
| 0.17 | 0.8           |
| 0.16 | 0.7.1, 0.7, 0.6 |
| 0.15 | 0.5           |
| 0.14 | 0.2, 0.3, 0.4 |
| 0.13 | 0.1           |

## License

Dual-licensed under either:

- [`MIT`](LICENSE-MIT): [http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT)
- [
  `Apache 2.0`](LICENSE-APACHE): [http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0)

At your option. This means that when using this crate in your game, you may choose which license to use.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as
defined in the Apache-2.0 license, shall be dually licensed as above, without any additional terms or conditions.
