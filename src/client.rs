use std::ops::{Deref, DerefMut};

use bevy::prelude::{Deref, DerefMut, FromWorld, Resource, World};
use rumqttc::{ConnectionError, Event, EventLoop};
use tokio::{runtime, sync::mpsc};

/// A struct that represents the settings for an MQTT connection
#[derive(Resource)]
pub struct MqttSetting {
    /// Options to configure the behaviour of MQTT connection
    pub mqtt_options: rumqttc::MqttOptions,
    /// specifies the capacity of the bounded async channel.
    pub cap: usize,
}

/// A component that represents an MQTT client
#[derive(Resource)]
pub struct MqttClient {
    #[allow(dead_code)]
    runtime: runtime::Handle,
    client: rumqttc::AsyncClient,
    pub(crate) from_async_event: mpsc::Receiver<Event>,
    pub(crate) from_async_error: mpsc::Receiver<ConnectionError>,
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

impl FromWorld for MqttClient {
    fn from_world(world: &mut World) -> Self {
        if world.get_resource::<AsyncRuntime>().is_none() {
            let async_runtime = runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .unwrap();
            world.insert_resource(AsyncRuntime(async_runtime));
        };

        let runtime = world.resource::<AsyncRuntime>();
        let setting = world
            .get_resource::<MqttSetting>()
            .expect("Mqtt setting should set");
        let (to_async_event, from_async_event) = mpsc::channel::<Event>(100);
        let (to_async_error, from_async_error) = mpsc::channel::<ConnectionError>(100);

        let (client, event_loop) = runtime.block_on(async {
            rumqttc::AsyncClient::new(setting.mqtt_options.clone(), setting.cap)
        });

        runtime.spawn(handle_event(event_loop, to_async_event, to_async_error));
        MqttClient {
            runtime: runtime.handle().clone(),
            client,
            from_async_event,
            from_async_error,
        }
    }
}

async fn handle_event(
    mut eventloop: EventLoop,
    tx_event: mpsc::Sender<Event>,
    tx_error: mpsc::Sender<ConnectionError>,
) {
    loop {
        match eventloop.poll().await {
            Ok(event) => tx_event.send(event).await.expect("send event failed"),
            Err(connection_err) => {
                tx_error
                    .send(connection_err)
                    .await
                    .expect("send error failed");
            }
        }
    }
}

#[derive(Resource, Deref, DerefMut)]
pub struct AsyncRuntime(pub(crate) runtime::Runtime);

/// A component that represents an MQTT connection
#[derive(Deref, DerefMut)]
pub struct MqttConnection(pub rumqttc::Connection);
