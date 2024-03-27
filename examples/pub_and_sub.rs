use bevy::prelude::*;
use bevy::time::common_conditions::on_timer;
use bevy_mqtt::rumqttc::{MqttOptions, QoS};

use bevy_mqtt::prelude::*;

fn main() {
    App::new()
        .insert_resource(MqttSetting {
            mqtt_options: MqttOptions::new("rumqtt-sync", "127.0.0.1", 1883),
            cap: 10,
        })
        .add_plugins((MinimalPlugins, MqttPlugin)).add_systems(Update, (handle_message, handle_error))
        .add_systems(Startup, sub_topic)
        .add_systems(Update, publish_message.run_if(on_timer(std::time::Duration::from_secs(1))))
        .run();
}

fn handle_message(mut mqtt_event: EventReader<MqttEvent>) {
    for event in mqtt_event.read() {
        println!("Received: {:?}", event.0);
    }
}

fn handle_error(mut error_events: EventReader<MqttError>) {
    for error in error_events.read() {
        println!("Error: {:?}", error);
    }
}

fn sub_topic(mut mqtt_client: Res<MqttClient>) {
    mqtt_client.subscribe("hello/+/world", QoS::AtMostOnce).unwrap();
}

fn publish_message(mut mqtt_client: Res<MqttClient>) {
    for i in 0..3 {
        let payload = vec![1; i];
        let topic = format!("hello/{i}/world");
        let qos = QoS::AtLeastOnce;

        mqtt_client.publish(topic, qos, false, payload).unwrap();
    }
}