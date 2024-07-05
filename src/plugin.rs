use bevy::prelude::*;

use crate::{
    client::MqttClient,
    events::{MqttError, MqttEvent},
    systems::recv_connection,
};

#[derive(Default)]
pub struct MqttPlugin;

impl Plugin for MqttPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MqttClient>();
        app.add_event::<MqttEvent>()
            .add_event::<MqttError>()
            .add_systems(Update, recv_connection);
    }
}
