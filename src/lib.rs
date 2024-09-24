//! A Bevy plugin for MQTT

use bevy_app::{App, Plugin, Update};
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::prelude::*;
use bevy_log::{debug, error, trace};
use bevy_state::{
    app::{AppExtStates, StatesPlugin},
    prelude::in_state,
    state::{NextState, States},
};
use bevy_tokio_tasks::{TokioTasksPlugin, TokioTasksRuntime};
use bytes::Bytes;
use kanal::{bounded_async, AsyncReceiver};
use regex::Regex;
pub use rumqttc;
use rumqttc::{ClientError, ConnectionError, QoS};
use std::ops::{Deref, DerefMut};

#[derive(Default)]
pub struct MqttPlugin;

impl Plugin for MqttPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<StatesPlugin>() {
            app.add_plugins(StatesPlugin);
        }
        if !app.is_plugin_added::<TokioTasksPlugin>() {
            app.add_plugins(TokioTasksPlugin::default());
        }

        app.init_state::<MqttClientState>()
            .add_event::<MqttEvent>()
            .add_event::<MqttConnectError>()
            .add_event::<MqttClientError>()
            .add_event::<MqttPublishOutgoing>()
            .add_event::<MqttPublishPacket>()
            .add_event::<DisconnectMqttClient>()
            .add_systems(Update, spawn_client.run_if(resource_added::<MqttSetting>))
            .add_systems(
                Update,
                (handle_mqtt_events, dispatch_publish_to_topic)
                    .run_if(resource_exists::<MqttClient>),
            )
            .add_systems(
                Update,
                (handle_outgoing_publish, handle_disconnect_event)
                    .run_if(in_state(MqttClientState::Connected)),
            )
            .observe(on_add_subscribe)
            .observe(on_remove_subscribe);
    }
}

/// A struct that represents the settings for an MQTT connection
#[derive(Resource, Clone)]
pub struct MqttSetting {
    /// Options to configure the behaviour of MQTT connection
    pub mqtt_options: rumqttc::MqttOptions,
    /// specifies the capacity of the bounded async channel.
    pub cap: usize,
}

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash, States)]
pub enum MqttClientState {
    #[default]
    Disconnected,
    Connected,
}

/// A component that represents an MQTT client
#[derive(Resource)]
pub struct MqttClient {
    client: rumqttc::AsyncClient,
    from_async_event: AsyncReceiver<rumqttc::Event>,
    from_async_error: AsyncReceiver<ConnectionError>,
}

impl Deref for MqttClient {
    type Target = rumqttc::AsyncClient;

    fn deref(&self) -> &Self::Target {
        &self.client
    }
}

impl DerefMut for MqttClient {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.client
    }
}

/// A wrapper around rumqttc::Event
#[derive(Debug, Clone, PartialEq, Eq, Deref, DerefMut, Event)]
pub struct MqttEvent(pub rumqttc::Event);

/// A wrapper around rumqttc::ConnectionError
#[derive(Debug, Deref, DerefMut, Event)]
pub struct MqttConnectError(pub ConnectionError);

/// A wrapper around rumqttc::ClientError
#[derive(Debug, Deref, DerefMut, Event)]
pub struct MqttClientError(pub ClientError);

#[derive(Debug, Event)]
pub struct MqttPublishOutgoing {
    pub topic: String,
    pub qos: QoS,
    pub retain: bool,
    pub payload: Vec<u8>,
}

#[derive(Debug, Event)]
pub struct MqttPublishPacket {
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
    client: Res<MqttClient>,
    mut mqtt_events: EventWriter<MqttEvent>,
    mut error_events: EventWriter<MqttConnectError>,
    mut next_state: ResMut<NextState<MqttClientState>>,
    mut publish_incoming: EventWriter<MqttPublishPacket>,
) {
    while let Ok(Some(event)) = client.from_async_event.try_recv() {
        match &event {
            rumqttc::Event::Incoming(rumqttc::Incoming::ConnAck(_)) => {
                next_state.set(MqttClientState::Connected);
            }
            rumqttc::Event::Incoming(rumqttc::Incoming::Disconnect) => {
                next_state.set(MqttClientState::Disconnected);
            }
            rumqttc::Event::Incoming(rumqttc::Incoming::Publish(publish)) => {
                publish_incoming.send(MqttPublishPacket {
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
        mqtt_events.send(MqttEvent(event));
    }

    while let Ok(Some(err)) = client.from_async_error.try_recv() {
        next_state.set(MqttClientState::Disconnected);
        error_events.send(MqttConnectError(err));
    }
}

/// spawn mqtt client by setting
fn spawn_client(setting: Res<MqttSetting>, runtime: Res<TokioTasksRuntime>) {
    let (to_async_event, from_async_event) = bounded_async::<rumqttc::Event>(100);
    let (to_async_error, from_async_error) = bounded_async::<ConnectionError>(100);
    let setting = setting.clone();
    runtime.spawn_background_task(move |mut ctx| async move {
        let (client, mut event_loop) =
            rumqttc::AsyncClient::new(setting.mqtt_options.clone(), setting.cap);

        ctx.run_on_main_thread(move |ctx| {
            let world = ctx.world;
            world.insert_resource(MqttClient {
                client,
                from_async_event,
                from_async_error,
            });
        })
        .await;
        debug!(
            "Mqtt client connecting_to {:?}",
            setting.mqtt_options.broker_address()
        );
        loop {
            match event_loop.poll().await {
                Ok(event) => {
                    let _ = to_async_event.send(event).await;
                }
                Err(connection_err) => {
                    error!("Mqtt client connection error: {:?}", connection_err);
                    let _ = to_async_error.send(connection_err).await;
                    // auto reconnect after 10 seconds
                    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                }
            }
        }
    });
}

/// A component to store the topic and qos to subscribe
#[derive(Debug, Clone, Component)]
pub struct SubscribeTopic {
    topic: String,
    re: Regex,
    qos: QoS,
}

impl SubscribeTopic {
    pub fn new(topic: impl ToString, qos: QoS) -> Self {
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

    pub fn qos(&self) -> QoS {
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
    topic_query: Query<(Entity, &SubscribeTopic)>,
    mut commands: Commands,
) {
    for packet in publish_incoming.read() {
        let mut match_entities = vec![];
        for (e, subscribed_topic) in topic_query.iter() {
            if subscribed_topic.matches(&packet.topic) {
                trace!(
                    "{} Received publish packet: {:?}",
                    subscribed_topic.topic(),
                    packet
                );
                match_entities.push(e);
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

fn on_add_subscribe(
    trigger: Trigger<OnAdd, SubscribeTopic>,
    query: Query<&SubscribeTopic>,
    client: Res<MqttClient>,
    mut client_error: EventWriter<MqttClientError>,
) {
    let subscribe = query.get(trigger.entity()).unwrap();
    debug!("subscribe to {:?}", subscribe.topic);
    let _ = client
        .try_subscribe(subscribe.topic.clone(), subscribe.qos)
        .map_err(|e| client_error.send(MqttClientError(e)));
}

fn on_remove_subscribe(
    trigger: Trigger<OnRemove, SubscribeTopic>,
    query: Query<&SubscribeTopic>,
    client: Res<MqttClient>,
    mut client_error: EventWriter<MqttClientError>,
) {
    let subscribe = query.get(trigger.entity()).unwrap();
    debug!("unsubscribe to {:?}", subscribe.topic);
    let _ = client
        .try_unsubscribe(subscribe.topic.clone())
        .map_err(|e| client_error.send(MqttClientError(e)));
}

fn handle_outgoing_publish(
    mut pub_events: EventReader<MqttPublishOutgoing>,
    client: Res<MqttClient>,
    mut client_error: EventWriter<MqttClientError>,
) {
    for event in pub_events.read() {
        let _ = client
            .try_publish(
                event.topic.clone(),
                event.qos,
                event.retain,
                event.payload.clone(),
            )
            .map_err(|e| client_error.send(MqttClientError(e)));
    }
}

fn handle_disconnect_event(
    client: Res<MqttClient>,
    mut disconnect_events: EventReader<DisconnectMqttClient>,
) {
    for _ in disconnect_events.read() {
        client.try_disconnect().expect("disconnect failed");
    }
}

#[test]
fn test_topic_matches() {
    let subscribe = SubscribeTopic::new("hello/+/world".to_string(), QoS::AtMostOnce);
    assert!(subscribe.matches("hello/1/world"));
}
