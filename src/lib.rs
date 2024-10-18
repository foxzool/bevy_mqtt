//! A Bevy plugin for MQTT

use bevy_app::{App, Plugin, Update};
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::prelude::*;
use bevy_hierarchy::{HierarchyQueryExt, Parent};
use bevy_log::{debug, trace};
use bevy_reflect::Reflect;
use bytes::Bytes;
use flume::{bounded, Receiver};
use regex::Regex;
pub use rumqttc;
use rumqttc::{qos, ClientError, ConnectionError, QoS, SubscribeFilter};
use std::collections::VecDeque;
use std::ops::{Deref, DerefMut};
use std::thread;

#[derive(Default)]
pub struct MqttPlugin;

impl Plugin for MqttPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<MqttEvent>()
            .add_event::<MqttConnectError>()
            .add_event::<MqttClientError>()
            .add_event::<MqttPublishOutgoing>()
            .add_event::<MqttPublishPacket>()
            .add_event::<DisconnectMqttClient>()
            .register_type::<SubscribeTopic>()
            .add_systems(
                Update,
                (on_added_setting_component, pending_subscribe_topic),
            )
            .add_systems(
                Update,
                (
                    handle_mqtt_events,
                    dispatch_publish_to_topic,
                    on_add_subscribe,
                ),
            )
            .observe(on_remove_subscribe);
    }
}

/// A struct that represents the settings for an MQTT connection
#[derive(Component, Clone)]
pub struct MqttSetting {
    /// Options to configure the behaviour of MQTT connection
    pub mqtt_options: rumqttc::MqttOptions,
    /// specifies the capacity of the bounded async channel.
    pub cap: usize,
}

/// A component that represents an MQTT client
#[derive(Component, Clone)]
pub struct MqttClient {
    client: rumqttc::Client,
    event_rx: Receiver<rumqttc::Event>,
    error_rx: Receiver<ConnectionError>,
    pedding_subscribes: Vec<SubscribeFilter>,
}

impl Deref for MqttClient {
    type Target = rumqttc::Client;

    fn deref(&self) -> &Self::Target {
        &self.client
    }
}

impl DerefMut for MqttClient {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.client
    }
}

/// A component to mark that an MQTT client is connected
#[derive(Component)]
pub struct MqttClientConnected;

/// A wrapper around rumqttc::Event
#[derive(Debug, Clone, PartialEq, Eq, Event)]
pub struct MqttEvent {
    pub entity: Entity,
    pub event: rumqttc::Event,
}

/// A wrapper around rumqttc::ConnectionError
#[derive(Debug, Event)]
pub struct MqttConnectError {
    pub entity: Entity,
    pub error: ConnectionError,
}

/// A wrapper around rumqttc::ClientError
#[derive(Debug, Event)]
pub struct MqttClientError {
    pub entity: Entity,
    pub error: ClientError,
}

#[derive(Debug, Event)]
pub struct MqttPublishOutgoing {
    pub topic: String,
    pub qos: QoS,
    pub retain: bool,
    pub payload: Vec<u8>,
}

#[derive(Debug, Event)]
pub struct MqttPublishPacket {
    pub entity: Entity,
    pub dup: bool,
    pub qos: QoS,
    pub retain: bool,
    pub topic: String,
    pub pkid: u16,
    pub payload: Bytes,
}

/// AN event to disconnect an MQTT client
#[derive(Event)]
pub struct DisconnectMqttClient;

fn handle_mqtt_events(
    clients: Query<(Entity, &MqttClient, &MqttSetting)>,
    mut commands: Commands,
    mut mqtt_events: EventWriter<MqttEvent>,
    mut error_events: EventWriter<MqttConnectError>,
    mut publish_incoming: EventWriter<MqttPublishPacket>,
) {
    for (entity, client, setting) in clients.iter() {
        while let Ok(event) = client.event_rx.try_recv() {
            match &event {
                rumqttc::Event::Incoming(rumqttc::Incoming::ConnAck(_)) => {
                    debug!(
                        "Mqtt client connected to {:?}",
                        setting.mqtt_options.broker_address()
                    );
                    commands.entity(entity).insert(MqttClientConnected);
                }
                rumqttc::Event::Incoming(rumqttc::Incoming::Disconnect) => {
                    commands.entity(entity).remove::<MqttClientConnected>();
                }
                rumqttc::Event::Incoming(rumqttc::Incoming::Publish(publish)) => {
                    publish_incoming.send(MqttPublishPacket {
                        entity,
                        dup: publish.dup,
                        qos: publish.qos,
                        retain: publish.retain,
                        topic: publish.topic.clone(),
                        pkid: publish.pkid,
                        payload: publish.payload.clone(),
                    });
                }
                rumqttc::Event::Incoming(_) | rumqttc::Event::Outgoing(_) => {}
            }
            mqtt_events.send(MqttEvent {
                entity,
                event: event.clone(),
            });
        }

        while let Ok(error) = client.error_rx.try_recv() {
            commands.entity(entity).remove::<MqttClientConnected>();
            error_events.send(MqttConnectError { entity, error });
        }
    }
}

/// spawn mqtt client by setting component
fn on_added_setting_component(
    setting_query: Query<(Entity, &MqttSetting), Added<MqttSetting>>,
    mut commands: Commands,
) {
    for (entity, setting) in setting_query.iter() {
        let (to_async_event, from_async_event) = bounded::<rumqttc::Event>(100);
        let (to_async_error, from_async_error) = bounded::<ConnectionError>(100);

        let (client, mut connection) =
            rumqttc::Client::new(setting.mqtt_options.clone(), setting.cap);

        thread::spawn(move || {
            for notification in connection.iter() {
                match notification {
                    Ok(event) => {
                        let _ = to_async_event.send(event);
                    }
                    Err(connection_err) => {
                        let _ = to_async_error.send(connection_err);
                        // auto reconnect after 5 seconds
                        thread::sleep(std::time::Duration::from_secs(5));
                    }
                }
            }
        });

        commands.entity(entity).insert(MqttClient {
            client,
            event_rx: from_async_event,
            error_rx: from_async_error,
            pedding_subscribes: vec![],
        });
    }
}

/// A component to store the topic and qos to subscribe
#[derive(Debug, Clone, Reflect, Component)]
#[reflect(from_reflect = false)]
pub struct SubscribeTopic {
    topic: String,
    qos: u8,
    #[reflect(ignore)]
    re: Regex,
}

/// A component to store the packet payload cache
#[derive(Debug, Component, Default, Deref, DerefMut)]
pub struct PacketCache(pub VecDeque<Bytes>);

impl SubscribeTopic {
    pub fn new(topic: impl ToString, qos: u8) -> Self {
        let topic = topic.to_string();
        let regex_pattern = topic.replace("+", "[^/]+").replace("#", ".+");
        let re = Regex::new(&format!("^{}$", regex_pattern)).unwrap();
        Self { topic, re, qos }
    }

    pub fn matches(&self, topic: &str) -> bool {
        self.re.is_match(topic)
    }

    pub fn topic(&self) -> &str {
        &self.topic
    }

    pub fn qos(&self) -> u8 {
        self.qos
    }
}

#[derive(Debug, Event)]
pub struct TopicMessage {
    pub topic: String,
    pub payload: Bytes,
}

fn dispatch_publish_to_topic(
    mut publish_incoming: EventReader<MqttPublishPacket>,
    mut topic_query: Query<(Entity, &SubscribeTopic, Option<&mut PacketCache>)>,
    parent_query: Query<&Parent>,
    mut commands: Commands,
) {
    for packet in publish_incoming.read() {
        let mut match_entities = vec![];
        for (e, subscribed_topic, opt_packet_cache) in topic_query.iter_mut() {
            if subscribed_topic.matches(&packet.topic) {
                trace!(
                    "{:?} {} Received matched packet",
                    e,
                    subscribed_topic.topic(),
                );
                match_entities.push(e);

                if let Some(mut message_cache) = opt_packet_cache {
                    message_cache.push_back(packet.payload.clone());
                }

                for ancestor in parent_query.iter_ancestors(e) {
                    match_entities.push(ancestor);
                }
            }
        }

        commands.trigger_targets(
            TopicMessage {
                topic: packet.topic.clone(),
                payload: packet.payload.clone(),
            },
            match_entities,
        );
    }
}

fn pending_subscribe_topic(
    mut clients: Query<(Entity, &mut MqttClient)>,
    mut client_error: EventWriter<MqttClientError>,
) {
    for (entity, mut client) in clients.iter_mut() {
        if client.pedding_subscribes.is_empty() {
            continue;
        }
        let sub_lists = client.pedding_subscribes.drain(..).collect::<Vec<_>>();
        let _ = client
            .subscribe_many(sub_lists)
            .map_err(|e| client_error.send(MqttClientError { entity, error: e }));
    }
}

fn on_add_subscribe(
    mut clients: Query<&mut MqttClient>,
    parent_query: Query<&Parent>,
    query: Query<(Entity, &SubscribeTopic), Added<SubscribeTopic>>,
) {
    for (entity, subscribe) in query.iter() {
        for ancestor in parent_query.iter_ancestors(entity) {
            if let Ok(mut client) = clients.get_mut(ancestor) {
                client.pedding_subscribes.push(SubscribeFilter::new(
                    subscribe.topic.clone(),
                    qos(subscribe.qos).unwrap_or(QoS::AtMostOnce),
                ));
            }
        }
    }
}

fn on_remove_subscribe(
    trigger: Trigger<OnRemove, SubscribeTopic>,
    parent_query: Query<&Parent>,
    clients: Query<&MqttClient>,
    query: Query<(&Parent, &SubscribeTopic)>,
    mut client_error: EventWriter<MqttClientError>,
) {
    let (parent, subscribe) = query.get(trigger.entity()).unwrap();
    for ancestor in parent_query.iter_ancestors(trigger.entity()) {
        if let Ok(client) = clients.get(ancestor) {
            let _ = client
                .try_unsubscribe(subscribe.topic.clone())
                .map_err(|e| {
                    client_error.send(MqttClientError {
                        entity: **parent,
                        error: e,
                    })
                });
        }
    }
}

#[test]
fn test_topic_matches() {
    let subscribe = SubscribeTopic::new("hello/+/world".to_string(), 0);
    assert!(subscribe.matches("hello/1/world"));
}
