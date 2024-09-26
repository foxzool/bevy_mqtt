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
use bevy::{prelude::*, time::common_conditions::on_timer};
use bevy_log::LogPlugin;
use bevy_mqtt::{
    rumqttc::QoS, MqttClient, MqttClientConnected, MqttClientError, MqttConnectError, MqttEvent,
    MqttPlugin, MqttSetting, SubscribeTopic, TopicMessage,
};
use rumqttc::{MqttOptions, Transport};
use std::time::Duration;

fn main() {
    App::new()
        .add_plugins((MinimalPlugins, MqttPlugin, LogPlugin::default()))
        .add_systems(Startup, setup_clients)
        .add_systems(Update, (sub_topic, handle_message, handle_error))
        .add_systems(
            Update,
            publish_message.run_if(on_timer(std::time::Duration::from_secs(1))),
        )
        .run();
}

fn setup_clients(mut commands: Commands) {
    commands.spawn(MqttSetting {
        mqtt_options: MqttOptions::new("bevy-mqtt-client", "127.0.0.1", 1883),
        cap: 10,
    });

    // spawn websocket client
    let mut mqtt_options = MqttOptions::new("mqtt-ws-client", "ws://127.0.0.1:8080", 8080);
    mqtt_options.set_transport(Transport::Ws);
    // mqtt_options.set_credentials("username", "password");
    mqtt_options.set_keep_alive(Duration::from_secs(5));

    commands.spawn((
        MqttSetting {
            mqtt_options,
            cap: 10,
        },
        WebsocketMqttClient,
    ));
}

// just a marker component for filter
#[derive(Component)]
struct WebsocketMqttClient;

/// this is a system that subscribes to a topic and handle the incoming messages
fn sub_topic(
    mqtt_client: Query<(Entity, &MqttClient, &MqttSetting), Added<MqttClientConnected>>,
    mut commands: Commands,
) {
    for (entity, client, setting) in mqtt_client.iter() {
        client
            .subscribe("hello".to_string(), QoS::AtMostOnce)
            .unwrap();

        let setting = setting.clone();
        let child_id = commands
            .spawn(SubscribeTopic::new("+/mqtt", QoS::AtMostOnce))
            .observe(move |topic_message: Trigger<TopicMessage>| {
                println!(
                    "{:?}: Topic: '+/mqtt' received : {:?}",
                    setting.mqtt_options.broker_address().clone(),
                    topic_message.event().payload
                );
            })
            .id();
        commands.entity(entity).add_child(child_id);
    }
}

/// this is global handler for all incoming messages
fn handle_message(mut mqtt_event: EventReader<MqttEvent>) {
    for event in mqtt_event.read() {
        match &event.event {
            rumqttc::Event::Incoming(income) => match income {
                rumqttc::Incoming::Publish(publish) => {
                    println!(
                        "Topic Component: {} Received: {:?}",
                        publish.topic, publish.payload
                    );
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

fn publish_message(mqtt_client: Query<&MqttClient, With<MqttClientConnected>>) {
    for client in mqtt_client.iter() {
        client
            .publish(
                "hello".to_string(),
                QoS::AtMostOnce,
                false,
                "mqtt".as_bytes(),
            )
            .unwrap();
        for i in 0..3 {
            client
                .publish(format!("{}/mqtt", i), QoS::AtMostOnce, false, b"hello")
                .unwrap();
        }
    }
}


```

## Supported Versions

| bevy | bevy_mqtt     |
|------|---------------|
| 0.14 | 0.2, 0.3, 0.4 |
| 0.13 | 0.1           |

## License

Dual-licensed under either

- MIT
- Apache 2.0
