use std::time::Duration;

use bevy::{prelude::*, time::common_conditions::on_timer};

use bevy_mqtt::{
    prelude::*,
    rumqttc::{MqttOptions, QoS, Transport},
};

fn main() {
    let mut mqtt_options =
        MqttOptions::new("clientId-aSziq39Bp3", "ws://127.0.0.1:8083/mqtt", 8083);
    mqtt_options.set_transport(Transport::Ws);
    mqtt_options.set_keep_alive(Duration::from_secs(60));
    App::new()
        .insert_resource(MqttSetting {
            mqtt_options,
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
        println!("Received: {:?}", event);
    }
}

fn handle_error(mut error_events: EventReader<MqttError>) {
    for error in error_events.read() {
        println!("Error: {:?}", error);
    }
}

fn sub_topic(mqtt_client: Res<MqttClient>) {
    mqtt_client
        .try_subscribe("hello/world", QoS::AtMostOnce)
        .unwrap();
}

fn publish_message(mqtt_client: Res<MqttClient>) {
    for i in 1..=10 {
        mqtt_client
            .try_publish("hello/world", QoS::AtMostOnce, false, vec![1; i])
            .unwrap();
        std::thread::sleep(Duration::from_secs_f32(0.1));
    }
}
