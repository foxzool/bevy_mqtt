//! A Bevy plugin for MQTT

use bevy_app::Plugin;
use bevy_app::{App, Update};
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::prelude::{
    resource_added, resource_exists, Event, EventReader, EventWriter, IntoSystemConfigs, Res,
    ResMut, Resource,
};
use bevy_log::debug;
use bevy_state::app::{AppExtStates, StatesPlugin};
use bevy_state::prelude::in_state;
use bevy_state::state::{NextState, States};
use bevy_tokio_tasks::{TokioTasksPlugin, TokioTasksRuntime};
use bytes::Bytes;
use kanal::{bounded_async, AsyncReceiver};
pub use rumqttc;
use rumqttc::{ConnectionError, QoS, SubscribeFilter};
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
            .add_event::<MqttError>()
            .add_event::<MqttSubTopicOutgoing>()
            .add_event::<MqttSubManyOutgoing>()
            .add_event::<MqttUnSubTopicOutgoing>()
            .add_event::<MqttPublishOutgoing>()
            .add_event::<MqttPublishPacket>()
            .add_event::<DisconnectMqttClient>()
            .add_systems(Update, spawn_client.run_if(resource_added::<MqttSetting>))
            .add_systems(
                Update,
                handle_mqtt_events.run_if(resource_exists::<MqttClient>),
            )
            .add_systems(
                Update,
                (
                    handle_sub_topic,
                    handle_outgoing_publish,
                    handle_disconnect_event,
                )
                    .distributive_run_if(in_state(MqttClientState::Connected)),
            );
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
pub struct MqttError(pub ConnectionError);

#[derive(Debug, Event)]
pub struct MqttSubTopicOutgoing {
    pub topic: String,
    pub qos: QoS,
}

#[derive(Debug, Event)]
pub struct MqttSubManyOutgoing {
    pub topics: Vec<SubscribeFilter>,
}

#[derive(Debug, Event)]
pub struct MqttUnSubTopicOutgoing {
    pub topic: String,
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
    pub dup: bool,
    pub qos: QoS,
    pub retain: bool,
    pub topic: String,
    pub pkid: u16,
    pub payload: Bytes,
}

/// A event to disconnect an MQTT client
#[derive(Event)]
pub struct DisconnectMqttClient;

fn handle_mqtt_events(
    client: Res<MqttClient>,
    mut mqtt_events: EventWriter<MqttEvent>,
    mut error_events: EventWriter<MqttError>,
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
        error_events.send(MqttError(err));
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
            "Mqtt client connect_to {:?}",
            setting.mqtt_options.broker_address()
        );
        loop {
            match event_loop.poll().await {
                Ok(event) => to_async_event.send(event).await.expect("send event failed"),
                Err(connection_err) => {
                    to_async_error
                        .send(connection_err)
                        .await
                        .expect("send error failed");
                }
            }
        }
    });
}

fn handle_sub_topic(
    mut sub_events: EventReader<MqttSubTopicOutgoing>,
    mut sub_many_events: EventReader<MqttSubManyOutgoing>,
    mut unsub_events: EventReader<MqttUnSubTopicOutgoing>,
    client: Res<MqttClient>,
) {
    for sub in sub_events.read() {
        debug!("subscribe to {:?}", sub.topic);
        client
            .try_subscribe(sub.topic.clone(), sub.qos)
            .expect("subscribe failed");
    }

    for sub in sub_many_events.read() {
        for topic in &sub.topics {
            debug!("subscribe to {:?}", topic.path);
        }
        client
            .try_subscribe_many(sub.topics.clone())
            .expect("subscribe many failed");
    }

    for unsub in unsub_events.read() {
        client
            .try_unsubscribe(unsub.topic.clone())
            .expect("unsubscribe failed");
    }
}

fn handle_outgoing_publish(
    mut pub_events: EventReader<MqttPublishOutgoing>,
    client: Res<MqttClient>,
) {
    for event in pub_events.read() {
        client
            .try_publish(
                event.topic.clone(),
                event.qos,
                event.retain,
                event.payload.clone(),
            )
            .expect("publish failed");
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
