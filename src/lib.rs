//! A Bevy plugin for MQTT

use bevy_app::{App, Plugin, Update};
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::prelude::*;
use bevy_log::{debug, trace};
use bevy_reflect::Reflect;
use bytes::Bytes;
use flume::{Receiver, bounded};
use regex::Regex;
pub use rumqttc;
use rumqttc::{ClientError, ConnectionError, QoS, SubscribeFilter, qos};
use std::{
    collections::VecDeque,
    ops::{Deref, DerefMut},
    thread,
};

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
            .add_systems(Update, (connect_mqtt_clients, pending_subscribe_topic))
            .add_systems(
                Update,
                (
                    handle_mqtt_events,
                    dispatch_publish_to_topic,
                    on_add_subscribe,
                ),
            )
            .add_observer(on_remove_subscribe);
    }
}

/// A struct that represents the settings for an MQTT connection
#[derive(Component, Clone)]
pub struct MqttSetting {
    /// Options to configure the behavior of MQTT connection
    pub mqtt_options: rumqttc::MqttOptions,
    /// Specifies the capacity of the bounded async channel.
    pub cap: usize,
}

/// A component that represents an MQTT client
#[derive(Component, Clone)]
pub struct MqttClient {
    client: rumqttc::Client,
    event_rx: Receiver<rumqttc::Event>,
    error_rx: Receiver<ConnectionError>,
    pending_subscribes: Vec<SubscribeFilter>,
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

/// An event to disconnect an MQTT client
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
                    publish_incoming.write(MqttPublishPacket {
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
            mqtt_events.write(MqttEvent {
                entity,
                event: event.clone(),
            });
        }

        while let Ok(error) = client.error_rx.try_recv() {
            // When connection error occurs, remove MqttClient and MqttClientConnected
            // This will trigger connect_mqtt_clients system to rebuild the client on next
            // frame
            commands
                .entity(entity)
                .remove::<(MqttClient, MqttClientConnected)>();
            error_events.write(MqttConnectError { entity, error });
        }
    }
}

/// Connect MQTT clients for settings that don't have a client yet
/// This handles both initial connections and reconnections after errors
fn connect_mqtt_clients(
    // Query for settings that don't have a client yet (initial or after error)
    setting_query: Query<(Entity, &MqttSetting), Without<MqttClient>>,
    mut commands: Commands,
) {
    for (entity, setting) in setting_query.iter() {
        debug!(
            "Creating MQTT client for {:?}",
            setting.mqtt_options.broker_address()
        );

        // Use the setting's cap for channel capacity instead of hardcoded 100
        let (to_async_event, from_async_event) = bounded::<rumqttc::Event>(setting.cap);
        let (to_async_error, from_async_error) = bounded::<ConnectionError>(setting.cap);

        let (client, mut connection) =
            rumqttc::Client::new(setting.mqtt_options.clone(), setting.cap);

        // Clone senders for the thread
        let event_sender = to_async_event.clone();
        let error_sender = to_async_error.clone();

        thread::spawn(move || {
            // Process connection events until error or channel disconnect
            for notification in connection.iter() {
                match notification {
                    Ok(event) => {
                        // If send fails, the receiver is dropped, thread should exit
                        if event_sender.send(event).is_err() {
                            trace!("MQTT event channel closed, exiting thread");
                            return;
                        }
                    }
                    Err(connection_err) => {
                        // Send the error and exit the thread
                        // The main thread will handle reconnection by recreating the client
                        let _ = error_sender.send(connection_err);
                        trace!("MQTT connection error, exiting thread for reconnection");
                        return;
                    }
                }
            }
            // If connection.iter() ends naturally, also exit
            trace!("MQTT connection iterator ended, exiting thread");
        });

        commands.entity(entity).insert(MqttClient {
            client,
            event_rx: from_async_event,
            error_rx: from_async_error,
            pending_subscribes: vec![],
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
    pub fn new(topic: impl ToString, qos: u8) -> Result<Self, regex::Error> {
        let topic = topic.to_string();
        let regex_pattern = topic.replace("+", "[^/]+").replace("#", ".+");
        let re = Regex::new(&format!("^{}$", regex_pattern))?;
        Ok(Self { topic, re, qos })
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
    parent_query: Query<&ChildOf>,
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
        if client.pending_subscribes.is_empty() {
            continue;
        }
        let sub_lists = client.pending_subscribes.drain(..).collect::<Vec<_>>();
        let _ = client
            .subscribe_many(sub_lists)
            .map_err(|e| client_error.write(MqttClientError { entity, error: e }));
    }
}

fn on_add_subscribe(
    mut clients: Query<&mut MqttClient>,
    parent_query: Query<&ChildOf>,
    query: Query<(Entity, &SubscribeTopic), Added<SubscribeTopic>>,
) {
    for (entity, subscribe) in query.iter() {
        for ancestor in parent_query.iter_ancestors(entity) {
            if let Ok(mut client) = clients.get_mut(ancestor) {
                client.pending_subscribes.push(SubscribeFilter::new(
                    subscribe.topic.clone(),
                    qos(subscribe.qos).unwrap_or(QoS::AtMostOnce),
                ));
            }
        }
    }
}

fn on_remove_subscribe(
    trigger: Trigger<OnRemove, SubscribeTopic>,
    parent_query: Query<&ChildOf>,
    clients: Query<(Entity, &MqttClient)>,
    subscribe_query: Query<&SubscribeTopic>,
    mut client_error: EventWriter<MqttClientError>,
) {
    let target_entity = trigger.target();
    
    // Try to get the SubscribeTopic data before it's removed
    let subscribe = if let Ok(s) = subscribe_query.get(target_entity) {
        s
    } else {
        // Component already removed or entity destroyed, nothing to do
        trace!("SubscribeTopic component not found for entity {:?}", target_entity);
        return;
    };

    // Look for MQTT clients in ancestor entities
    for ancestor in parent_query.iter_ancestors(target_entity) {
        if let Ok((client_entity, client)) = clients.get(ancestor) {
            if let Err(e) = client.try_unsubscribe(subscribe.topic.clone()) {
                client_error.write(MqttClientError {
                    entity: client_entity,
                    error: e,
                });
            }
        }
    }
    
    // Also check if the entity itself is a client
    if let Ok((client_entity, client)) = clients.get(target_entity) {
        if let Err(e) = client.try_unsubscribe(subscribe.topic.clone()) {
            client_error.write(MqttClientError {
                entity: client_entity,
                error: e,
            });
        }
    }
}

#[test]
fn test_topic_matches() {
    let subscribe = SubscribeTopic::new("hello/+/world".to_string(), 0).unwrap();
    assert!(subscribe.matches("hello/1/world"));
}

#[test]
fn test_invalid_topic_pattern() {
    // Test that invalid regex patterns are handled gracefully
    let result = SubscribeTopic::new("hello/[invalid", 0);
    assert!(result.is_err());
}
