use bevy::prelude::*;
use rumqttc::Client;

use crate::client::{MqttClient, MqttConnection, MqttSetting};
use crate::events::{MqttError, MqttEvent};
use crate::systems::recv_connection;

#[derive(Default)]
pub struct MqttPlugin;

impl Plugin for MqttPlugin {
    fn build(&self, app: &mut App) {
        if !app.world.is_resource_added::<MqttSetting>() {
            panic!("MqttSetting resource is required to use MqttPlugin")
        }
        let setting = app.world.get_resource::<MqttSetting>().unwrap();

        let (client, connection) = Client::new(setting.mqtt_options.clone(), setting.cap);

        app.insert_resource(MqttClient(client));
        app.insert_non_send_resource(MqttConnection(connection));
        app.add_event::<MqttEvent>()
            .add_event::<MqttError>()
            .add_systems(Update, recv_connection);
    }
}
