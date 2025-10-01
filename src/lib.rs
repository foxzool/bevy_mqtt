//! A Bevy plugin for MQTT

use bevy_app::{App, Plugin, Update};
// Removed unused imports: use bevy_derive::{Deref, DerefMut};
use bevy_ecs::prelude::*;
use bevy_ecs::{
    event::{EntityEvent, Event},
    message::Message,
    observer::On,
};
use bevy_log::{debug, trace};
use bytes::Bytes;
use flume::{bounded, Receiver};
use regex::Regex;
pub use rumqttc;
use rumqttc::{ClientError, ConnectionError, QoS, SubscribeFilter};
use std::{
    collections::VecDeque,
    ops::{Deref, DerefMut},
    panic, thread,
};

#[derive(Default)]
pub struct MqttPlugin;

impl Plugin for MqttPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<MqttEvent>()
            .add_message::<MqttConnectError>()
            .add_message::<MqttClientError>()
            .add_message::<MqttPublishOutgoing>()
            .add_message::<MqttPublishPacket>()
            .add_message::<DisconnectMqttClient>()
            // Note: SubscribeTopic cannot be registered for reflection due to QoS enum
            .add_systems(Update, (connect_mqtt_clients, pending_subscribe_topic))
            .add_systems(
                Update,
                (
                    handle_mqtt_events,
                    dispatch_publish_to_topic,
                    on_add_subscribe,
                    handle_outgoing_publish,
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
#[derive(Debug, Clone, PartialEq, Eq, Event, Message)]
pub struct MqttEvent {
    pub entity: Entity,
    pub event: rumqttc::Event,
}

/// A wrapper around rumqttc::ConnectionError
#[derive(Debug, Event, Message)]
pub struct MqttConnectError {
    pub entity: Entity,
    pub error: ConnectionError,
}

/// A wrapper around rumqttc::ClientError
#[derive(Debug, Message)]
pub struct MqttClientError {
    pub entity: Entity,
    pub error: ClientError,
}

#[derive(Debug, Message)]
pub struct MqttPublishOutgoing {
    /// The entity of the MqttClient that should publish the message
    pub entity: Entity,
    pub topic: String,
    pub qos: QoS,
    pub retain: bool,
    pub payload: Vec<u8>,
}

#[derive(Debug, Event, Message)]
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
#[derive(Event, Message)]
pub struct DisconnectMqttClient;

fn handle_mqtt_events(
    clients: Query<(Entity, &MqttClient, &MqttSetting)>,
    mut commands: Commands,
    mut mqtt_events: MessageWriter<MqttEvent>,
    mut error_events: MessageWriter<MqttConnectError>,
    mut publish_incoming: MessageWriter<MqttPublishPacket>,
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
            // When connection error occurs, remove both MqttClient and MqttClientConnected
            // components. These components MUST be removed together to maintain
            // consistency:
            // - MqttClient: Contains the actual client and channels
            // - MqttClientConnected: Marks the client as connected
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
            // Wrap the entire thread logic in panic handling to ensure robust error
            // reporting
            let thread_result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
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
                            if error_sender.send(connection_err).is_err() {
                                // This can happen if the MqttClient component is removed and the
                                // receiver is dropped.
                                // It's an expected condition for shutdown.
                                trace!("MQTT error channel closed, exiting thread.");
                            }
                            trace!("MQTT connection error, exiting thread for reconnection");
                            return;
                        }
                    }
                }
                // If connection.iter() ends naturally, also exit
                trace!("MQTT connection iterator ended, exiting thread");
            }));

            // Handle any panics that occurred in the thread
            if let Err(panic_info) = thread_result {
                let panic_message = if let Some(s) = panic_info.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = panic_info.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "Unknown panic occurred".to_string()
                };

                // Try to send a synthetic connection error to signal the panic to the main
                // thread If this fails, the main thread will eventually detect
                // the thread termination
                let synthetic_error = ConnectionError::Io(std::io::Error::other(format!(
                    "MQTT thread panicked: {}",
                    panic_message
                )));

                let _ = error_sender.send(synthetic_error);
                debug!("MQTT thread panicked: {}", panic_message);
            }
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
#[derive(Debug, Clone, Component)]
pub struct SubscribeTopic {
    topic: String,
    qos: QoS,
    re: Regex,
}

/// A component to store the packet payload cache with capacity limit
#[derive(Debug, Component)]
pub struct PacketCache {
    pub packets: VecDeque<Bytes>,
    pub capacity: usize,
}

impl Default for PacketCache {
    fn default() -> Self {
        Self::new(100) // Default capacity of 100 messages
    }
}

impl PacketCache {
    /// Create a new PacketCache with specified capacity
    pub fn new(capacity: usize) -> Self {
        let safe_capacity = capacity.clamp(1, 1000); // Cap at 1000 for safety, minimum 1
        Self {
            packets: VecDeque::with_capacity(safe_capacity),
            capacity: safe_capacity,
        }
    }

    /// Push a new packet, removing oldest if at capacity
    pub fn push(&mut self, packet: Bytes) {
        if self.packets.len() >= self.capacity {
            self.packets.pop_front(); // Remove oldest packet
            trace!(
                "PacketCache at capacity {}, removing oldest packet",
                self.capacity
            );
        }
        self.packets.push_back(packet);
    }

    /// Get the number of cached packets
    pub fn len(&self) -> usize {
        self.packets.len()
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.packets.is_empty()
    }

    /// Get iterator over packets (oldest first)
    pub fn iter(&self) -> impl Iterator<Item = &Bytes> {
        self.packets.iter()
    }

    /// Clear all cached packets
    pub fn clear(&mut self) {
        self.packets.clear();
    }

    /// Get the most recent packet
    pub fn latest(&self) -> Option<&Bytes> {
        self.packets.back()
    }

    /// Get the oldest packet
    pub fn oldest(&self) -> Option<&Bytes> {
        self.packets.front()
    }
}

impl SubscribeTopic {
    pub fn new(topic: impl ToString, qos: QoS) -> Result<Self, regex::Error> {
        let topic = topic.to_string();

        // Escape regex metacharacters in topic, preserving MQTT wildcards + and #
        let escaped_topic = topic
            .chars()
            .map(|c| match c {
                // Preserve MQTT wildcards
                '+' | '#' => c.to_string(),
                // Escape regex metacharacters
                '.' | '^' | '$' | '*' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '\\' | '|' => {
                    format!("\\{}", c)
                }
                // Regular characters
                _ => c.to_string(),
            })
            .collect::<String>();

        // Convert MQTT wildcards to regex patterns
        let regex_pattern = escaped_topic.replace("+", "[^/]+").replace("#", ".+");
        let re = Regex::new(&format!("^{}$", regex_pattern))?;
        Ok(Self { topic, re, qos })
    }

    pub fn matches(&self, topic: &str) -> bool {
        self.re.is_match(topic)
    }

    pub fn topic(&self) -> &str {
        &self.topic
    }

    pub fn qos(&self) -> QoS {
        self.qos
    }
}

#[derive(Debug, EntityEvent)]
pub struct TopicMessage {
    #[event_target]
    pub target: Entity,
    pub topic: String,
    pub payload: Bytes,
}

fn dispatch_publish_to_topic(
    mut publish_incoming: MessageReader<MqttPublishPacket>,
    mut topic_query: Query<(Entity, &SubscribeTopic, Option<&mut PacketCache>)>,
    parent_query: Query<&ChildOf>,
    mut commands: Commands,
    // Performance optimization: Use Local<Vec<Entity>> to avoid allocating a new Vec on every
    // system run. The Vec is automatically reused across calls, and std::mem::take() at the
    // end ensures it starts empty for the next iteration, while transferring ownership to
    // trigger_targets().
    mut match_entities: Local<Vec<Entity>>,
) {
    for packet in publish_incoming.read() {
        // IMPORTANT: The vector is guaranteed to be empty from the previous iteration's
        // std::mem::take
        for (e, subscribed_topic, opt_packet_cache) in topic_query.iter_mut() {
            if subscribed_topic.matches(&packet.topic) {
                trace!(
                    "{:?} {} Received matched packet",
                    e,
                    subscribed_topic.topic(),
                );
                match_entities.push(e);

                if let Some(mut message_cache) = opt_packet_cache {
                    message_cache.push(packet.payload.clone());
                }

                for ancestor in parent_query.iter_ancestors(e) {
                    match_entities.push(ancestor);
                }
            }
        }

        if !match_entities.is_empty() {
            let entities = std::mem::take(&mut *match_entities);
            let topic = packet.topic.clone();
            let payload = packet.payload.clone();

            for entity in entities {
                let topic_clone = topic.clone();
                let payload_clone = payload.clone();
                commands
                    .entity(entity)
                    .trigger(move |target: Entity| TopicMessage {
                        target,
                        topic: topic_clone,
                        payload: payload_clone,
                    });
            }
        }
    }
}

fn pending_subscribe_topic(
    mut clients: Query<(Entity, &mut MqttClient)>,
    mut client_error: MessageWriter<MqttClientError>,
) {
    for (entity, mut client) in clients.iter_mut() {
        if client.pending_subscribes.is_empty() {
            continue;
        }
        let sub_lists = client.pending_subscribes.drain(..).collect::<Vec<_>>();
        if let Err(e) = client.subscribe_many(sub_lists) {
            client_error.write(MqttClientError { entity, error: e });
        }
    }
}

fn on_add_subscribe(
    mut clients: Query<&mut MqttClient>,
    parent_query: Query<&ChildOf>,
    query: Query<(Entity, &SubscribeTopic), Added<SubscribeTopic>>,
) {
    for (entity, subscribe) in query.iter() {
        let mut found_client = false;
        for ancestor in parent_query.iter_ancestors(entity) {
            if let Ok(mut client) = clients.get_mut(ancestor) {
                client
                    .pending_subscribes
                    .push(SubscribeFilter::new(subscribe.topic.clone(), subscribe.qos));
                found_client = true;
                break; // Found client, no need to check more ancestors
            }
        }

        if !found_client {
            debug!(
                "No MQTT client found for SubscribeTopic entity {:?} with topic '{}'",
                entity, subscribe.topic
            );
        }
    }
}

fn on_remove_subscribe(
    trigger: On<Remove, SubscribeTopic>,
    parent_query: Query<&ChildOf>,
    clients: Query<(Entity, &MqttClient)>,
    subscribe_query: Query<&SubscribeTopic>,
    mut client_error: MessageWriter<MqttClientError>,
) {
    let target_entity = trigger.event().entity;

    // Try to get the SubscribeTopic data before it's removed
    let subscribe = if let Ok(s) = subscribe_query.get(target_entity) {
        s
    } else {
        // Component already removed or entity destroyed, nothing to do
        trace!(
            "SubscribeTopic component not found for entity {:?}",
            target_entity
        );
        return;
    };

    // Look for MQTT clients in the entity itself and its ancestors
    for entity_to_check in
        std::iter::once(target_entity).chain(parent_query.iter_ancestors(target_entity))
    {
        if let Ok((client_entity, client)) = clients.get(entity_to_check)
            && let Err(e) = client.try_unsubscribe(subscribe.topic.clone())
        {
            client_error.write(MqttClientError {
                entity: client_entity,
                error: e,
            });
        }
    }
}

/// Handle outgoing publish events to send messages via MQTT clients
fn handle_outgoing_publish(
    mut events: MessageReader<MqttPublishOutgoing>,
    clients: Query<&MqttClient>,
    mut client_error: MessageWriter<MqttClientError>,
) {
    for event in events.read() {
        if let Ok(client) = clients.get(event.entity) {
            trace!(
                "Publishing message to topic '{}' via client {:?}",
                event.topic, event.entity
            );

            if let Err(e) = client.publish(
                event.topic.clone(),
                event.qos,
                event.retain,
                event.payload.clone(),
            ) {
                client_error.write(MqttClientError {
                    entity: event.entity,
                    error: e,
                });
            }
        } else {
            debug!(
                "Cannot publish to topic '{}': MqttClient not found for entity {:?}",
                event.topic, event.entity
            );
        }
    }
}

#[test]
fn test_topic_matches() {
    let subscribe = SubscribeTopic::new("hello/+/world".to_string(), QoS::AtMostOnce).unwrap();
    assert!(subscribe.matches("hello/1/world"));
}

#[test]
fn test_invalid_topic_pattern() {
    // Test that the regex escaping now prevents invalid patterns from causing
    // errors Previously "hello/[invalid" would fail, but now it's escaped as
    // "hello/\[invalid"
    let result = SubscribeTopic::new("hello/[invalid", QoS::AtMostOnce);
    assert!(result.is_ok());

    // Verify the escaped pattern works correctly
    let subscribe = result.unwrap();
    assert!(subscribe.matches("hello/[invalid"));
    assert!(!subscribe.matches("hello/invalid"));
}

#[test]
fn test_topic_regex_escaping() {
    // Test that regex metacharacters are properly escaped
    let subscribe = SubscribeTopic::new("test/topic.with*special[chars]", QoS::AtMostOnce).unwrap();
    assert!(subscribe.matches("test/topic.with*special[chars]"));
    assert!(!subscribe.matches("test/topicXwithXspecialXchars"));

    // Test MQTT wildcards still work after escaping
    let subscribe_wildcard = SubscribeTopic::new("test/+/special.*", QoS::AtMostOnce).unwrap();
    assert!(subscribe_wildcard.matches("test/anything/special.*"));
    assert!(!subscribe_wildcard.matches("test/anything/specialXX"));
}

#[test]
fn test_packet_cache_capacity_limit() {
    let mut cache = PacketCache::new(2);

    // Test basic functionality
    assert_eq!(cache.len(), 0);
    assert!(cache.is_empty());

    // Add first packet
    cache.push(Bytes::from("packet1"));
    assert_eq!(cache.len(), 1);
    assert!(!cache.is_empty());
    assert_eq!(cache.latest(), Some(&Bytes::from("packet1")));
    assert_eq!(cache.oldest(), Some(&Bytes::from("packet1")));

    // Add second packet
    cache.push(Bytes::from("packet2"));
    assert_eq!(cache.len(), 2);
    assert_eq!(cache.latest(), Some(&Bytes::from("packet2")));
    assert_eq!(cache.oldest(), Some(&Bytes::from("packet1")));

    // Add third packet - should remove oldest
    cache.push(Bytes::from("packet3"));
    assert_eq!(cache.len(), 2); // Still only 2 packets
    assert_eq!(cache.latest(), Some(&Bytes::from("packet3")));
    assert_eq!(cache.oldest(), Some(&Bytes::from("packet2"))); // packet1 was removed

    // Test clear
    cache.clear();
    assert_eq!(cache.len(), 0);
    assert!(cache.is_empty());
}

#[test]
fn test_packet_cache_minimum_capacity() {
    // Test that capacity is at least 1
    let cache = PacketCache::new(0);
    assert_eq!(cache.capacity, 1);
}

#[test]
fn test_packet_cache_maximum_capacity() {
    // Test that capacity is capped at 1000
    let cache = PacketCache::new(2000);
    assert_eq!(cache.capacity, 1000); // The capacity field itself should be capped at 1000
    assert!(cache.packets.capacity() <= 1000); // And the VecDeque capacity is also capped
}

#[test]
fn test_packet_cache_capacity_consistency() {
    // Test that push respects the safe capacity limit, not the original user input
    let mut cache = PacketCache::new(5000); // Very large input capacity
    assert_eq!(cache.capacity, 1000); // Should be capped at 1000

    // Fill the cache beyond 1000 items should not be possible
    for i in 0..1500 {
        cache.push(Bytes::from(format!("packet{}", i)));
    }

    // Cache should never exceed the safe capacity limit of 1000
    assert_eq!(cache.len(), 1000);
    assert_eq!(cache.capacity, 1000);

    // The oldest packet should be packet500 (items 0-499 were evicted)
    assert_eq!(cache.oldest(), Some(&Bytes::from("packet500")));
    // The newest packet should be packet1499
    assert_eq!(cache.latest(), Some(&Bytes::from("packet1499")));
}
