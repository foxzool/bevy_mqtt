# bevy_mqtt

[![crates.io](https://img.shields.io/crates/v/bevy_mqtt)](https://crates.io/crates/bevy_mqtt)
[![MIT/Apache 2.0](https://img.shields.io/badge/license-MIT%2FApache-blue.svg)](https://github.com/Seldom-SE/seldom_pixel#license)
[![crates.io](https://img.shields.io/crates/d/bevy_mqtt)](https://crates.io/crates/bevy_mqtt)
[![CI](https://github.com/foxzool/bevy_mqtt/workflows/CI/badge.svg)](https://github.com/foxzool/bevy_mqtt/actions)
[![Documentation](https://docs.rs/bevy_mqtt/badge.svg)](https://docs.rs/bevy_mqtt)

A mqtt client Plugin for Bevy game engine.

## Example

```rust
use std::time::SystemTime;

use bevy::{prelude::*, time::common_conditions::on_timer};
use bincode::ErrorKind;
use serde::{Deserialize, Serialize};

use bevy_mqtt::prelude::*;
use bevy_mqtt::rumqttc::{MqttOptions, QoS};

#[derive(Serialize, Deserialize, Debug)]
struct Message {
    i: usize,
    time: SystemTime,
}

impl From<&Message> for Vec<u8> {
    fn from(value: &Message) -> Self {
        bincode::serialize(value).unwrap()
    }
}

impl From<Message> for Vec<u8> {
    fn from(value: Message) -> Self {
        bincode::serialize(&value).unwrap()
    }
}

impl TryFrom<&[u8]> for Message {
    type Error = Box<ErrorKind>;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        bincode::deserialize(value)
    }
}

fn main() {
    App::new()
        .insert_resource(MqttSetting {
            mqtt_options: MqttOptions::new("rumqtt-serde", "127.0.0.1", 1883),
            cap: 10,
        })
        .add_plugins((MinimalPlugins, MqttPlugin))
        .add_systems(Update, (handle_message, handle_error))
        .add_systems(Startup, sub_topic)
        .add_systems(
            Update,
            publish_message.run_if(on_timer(std::time::Duration::from_secs(1))),
        )
        .run();
}

fn handle_message(mut mqtt_event: EventReader<MqttEvent>) {
    for event in mqtt_event.read() {
        match &event.0 {
            rumqttc::Event::Incoming(income) => match income {
                rumqttc::Incoming::Publish(publish) => {
                    let message: Message = bincode::deserialize(&publish.payload).unwrap();
                    println!("Received: {:?}", message);
                }
                _ => {}
            },
            rumqttc::Event::Outgoing(_) => {}
        }
    }
}

fn handle_error(mut error_events: EventReader<MqttError>) {
    for error in error_events.read() {
        println!("Error: {:?}", error);
    }
}

fn sub_topic(mut mqtt_client: Res<MqttClient>) {
    mqtt_client
        .subscribe("hello/mqtt", QoS::AtMostOnce)
        .unwrap();
}

fn publish_message(mut mqtt_client: Res<MqttClient>) {
    for i in 0..3 {
        let message = Message {
            i,
            time: SystemTime::now(),
        };

        mqtt_client
            .publish("hello/mqtt", QoS::AtLeastOnce, false, message)
            .unwrap();
    }
}


```

## Supported Versions

| bevy | bevy_mqtt |
|------|-----------|
| 0.14 | 0.2       |
| 0.13 | 0.1       |

## License

Dual-licensed under either

- MIT
- Apache 2.0
