# bevy_mqtt

[![crates.io](https://img.shields.io/crates/v/bevy_mqtt)](https://crates.io/crates/bevy_mqtt)
[![MIT/Apache 2.0](https://img.shields.io/badge/license-MIT%2FApache-blue.svg)](https://github.com/Seldom-SE/seldom_pixel#license)
[![crates.io](https://img.shields.io/crates/d/bevy_mqtt)](https://crates.io/crates/bevy_mqtt)
[![CI](https://github.com/foxzool/bevy_mqtt/workflows/CI/badge.svg)](https://github.com/foxzool/bevy_mqtt/actions)
[![Documentation](https://docs.rs/bevy_mqtt/badge.svg)](https://docs.rs/bevy_mqtt)

A mqtt client Plugin for Bevy game engine.

## Example

first run as mqtt broker like [mosquitto](https://mosquitto.org/)

then run the example

```rust
use std::time::SystemTime;

use bevy::{prelude::*, time::common_conditions::on_timer};
use bevy_log::LogPlugin;
use bevy_mqtt::{
    rumqttc::{MqttOptions, QoS},
    MqttClient, MqttClientError, MqttClientState, MqttConnectError, MqttEvent, MqttPlugin,
    MqttPublishOutgoing, MqttSetting, SubscribeTopic, TopicMessage,
};
use bevy_state::prelude::OnEnter;
use bincode::ErrorKind;
use serde::{Deserialize, Serialize};

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
            mqtt_options: MqttOptions::new("mqtt-serde", "127.0.0.1", 1883),
            cap: 10,
        })
        .add_plugins((MinimalPlugins, MqttPlugin, LogPlugin::default()))
        .add_systems(Update, (handle_message, handle_error))
        .add_systems(
            OnEnter(MqttClientState::Connected),
            (sub_topic_direct, sub_topic_by_component),
        )
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
                    println!("Received Publish: {:?}", message);
                }
                _ => {
                    println!("Incoming: {:?}", income);
                }
            },
            rumqttc::Event::Outgoing(_) => {}
        }
    }
}

fn handle_error(
    mut connect_errors: EventReader<MqttConnectError>,
    mut client_errors: EventReader<MqttClientError>,
) {
    for error in connect_errors.read() {
        println!("connect Error: {:?}", error);
    }

    for error in client_errors.read() {
        println!("client Error: {:?}", error);
    }
}

/// there are two ways to subscribe to a topic
/// 1. Directly subscribe to a topic
/// 2. Subscribe to a topic by component
fn sub_topic_direct(client: Res<MqttClient>) {
    client
        .try_subscribe("hello/mqtt", QoS::AtMostOnce)
        .expect("subscribe failed");
}

fn sub_topic_by_component(mut commands: Commands) {
    commands
        .spawn(SubscribeTopic::new("+/mqtt", QoS::AtMostOnce))
        .observe(|topic_message: Trigger<TopicMessage>| {
            println!(
                "topic: {} received : {:?}",
                topic_message.event().topic,
                topic_message.event().payload
            );
        });
}

fn publish_message(mut pub_events: EventWriter<MqttPublishOutgoing>) {
    let mut list = vec![];
    for i in 0..3 {
        let message = Message {
            i,
            time: SystemTime::now(),
        };

        list.push(MqttPublishOutgoing {
            topic: "hello/mqtt".to_string(),
            qos: QoS::AtLeastOnce,
            retain: false,
            payload: message.into(),
        });
        list.push(MqttPublishOutgoing {
            topic: "bevy/mqtt".to_string(),
            qos: QoS::AtLeastOnce,
            retain: false,
            payload: Message {
                i: 999,
                time: SystemTime::now(),
            }
                .into(),
        });
    }

    pub_events.send_batch(list);
}

```

## Supported Versions

| bevy | bevy_mqtt |
|------|-----------|
| 0.14 | 0.2, 0.3  |
| 0.13 | 0.1       |

## License

Dual-licensed under either

- MIT
- Apache 2.0
