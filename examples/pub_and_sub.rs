use bevy::prelude::*;
use rumqttc::MqttOptions;

use bevy_mqtt::prelude::*;

fn main() {
    App::new()
        .insert_resource(MqttSetting {
            mqtt_options: MqttOptions::new("rumqtt-sync", "127.0.0.1", 1883),
            cap: 10,
        })
        .add_plugins((MinimalPlugins, MqttPlugin)).add_systems(Update, publish).run();
}

fn publish() {
    // println!("Publishing message");
}