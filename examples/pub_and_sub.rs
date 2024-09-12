use std::time::SystemTime;

use bevy::{prelude::*, time::common_conditions::on_timer};
use bevy_mqtt::{
    rumqttc::{MqttOptions, QoS},
    MqttClientState, MqttError, MqttEvent, MqttPlugin, MqttPublishOutgoing, MqttSetting,
    MqttSubTopicOutgoing,
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
        .add_plugins((MinimalPlugins, MqttPlugin))
        .add_systems(Update, (handle_message, handle_error))
        .add_systems(OnEnter(MqttClientState::Connected), sub_topic)
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
    }

    pub_events.send_batch(list);
}
