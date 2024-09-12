use std::time::Duration;

use bevy::{prelude::*, time::common_conditions::on_timer};
use bevy_mqtt::{
    rumqttc::{MqttOptions, QoS, Transport},
    MqttClientState, MqttError, MqttPlugin, MqttPublishOutgoing, MqttPublishPacket, MqttSetting,
    MqttSubTopicOutgoing,
};
use bevy_state::state::OnEnter;

fn main() {
    let mut mqtt_options = MqttOptions::new("clientId-aSziq39Bp3", "ws://127.0.0.1", 8083);
    mqtt_options.set_transport(Transport::Ws);
    mqtt_options.set_credentials("username", "password");
    mqtt_options.set_keep_alive(Duration::from_secs(60));
    App::new()
        .insert_resource(MqttSetting {
            mqtt_options,
            cap: 10,
        })
        .add_plugins((MinimalPlugins, MqttPlugin))
        .add_systems(OnEnter(MqttClientState::Connected), sub_topic)
        .add_systems(Update, (handle_mqtt_publish, handle_error))
        .add_systems(
            Update,
            publish_message.run_if(on_timer(std::time::Duration::from_secs(1))),
        )
        .run();
}

// fn handle_mqtt_event(mut mqtt_event: EventReader<MqttEvent>) {
//     for event in mqtt_event.read() {
//         println!("Received: {:?}", event);
//     }
// }

fn handle_mqtt_publish(mut mqtt_event: EventReader<MqttPublishPacket>) {
    for packet in mqtt_event.read() {
        println!("{:?}", packet);
    }
}

fn handle_error(mut error_events: EventReader<MqttError>) {
    for error in error_events.read() {
        println!("Error: {:?}", error);
    }
}

fn sub_topic(mut sub_events: EventWriter<MqttSubTopicOutgoing>) {
    sub_events.send(MqttSubTopicOutgoing {
        topic: "hello/mqtt".to_string(),
        qos: QoS::AtMostOnce,
    });
}

fn publish_message(mut pub_events: EventWriter<MqttPublishOutgoing>) {
    let mut list = vec![];
    for i in 1..=10 {
        list.push(MqttPublishOutgoing {
            topic: "hello/mqtt".to_string(),
            qos: QoS::AtLeastOnce,
            retain: false,
            payload: vec![1; i],
        });
    }

    pub_events.send_batch(list);
}
