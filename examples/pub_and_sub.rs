use bevy::{prelude::*, time::common_conditions::on_timer};
use bevy_log::LogPlugin;
use bevy_mqtt::{
    MqttClient, MqttClientConnected, MqttClientError, MqttConnectError, MqttEvent, MqttPlugin,
    MqttPublishOutgoing, MqttSetting, SubscribeTopic, TopicMessage, rumqttc::QoS,
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
            (
                publish_message.run_if(on_timer(Duration::from_secs(1))),
                publish_via_events.run_if(on_timer(Duration::from_secs(2))),
            ),
        )
        .run();
}

fn setup_clients(mut commands: Commands) {
    commands.spawn(MqttSetting {
        mqtt_options: MqttOptions::new("bevy-mqtt-client", "localhost", 1883),
        cap: 10,
    });

    // spawn websocket client
    let mut mqtt_options = MqttOptions::new("mqtt-ws-client", "ws://127.0.0.1:9001", 9001);
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
        // subscribe directly
        client
            .subscribe("hello".to_string(), QoS::AtMostOnce)
            .unwrap();

        let setting = setting.clone();
        let child_id = commands
            .spawn(SubscribeTopic::new("+/mqtt", QoS::AtMostOnce).expect("Invalid topic pattern"))
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

/// Example of using the new event-driven publish API
fn publish_via_events(
    mqtt_clients: Query<Entity, (With<MqttClient>, With<MqttClientConnected>)>,
    mut publish_events: EventWriter<MqttPublishOutgoing>,
) {
    for client_entity in mqtt_clients.iter() {
        // Send message via event - this is useful for decoupled systems
        publish_events.write(MqttPublishOutgoing {
            entity: client_entity,
            topic: "events/test".to_string(),
            qos: QoS::AtMostOnce,
            retain: false,
            payload: b"Hello from event system!".to_vec(),
        });
    }
}
