use bevy::prelude::{EventWriter, ResMut};

use crate::{
    client::MqttClient,
    events::{MqttError, MqttEvent},
};

pub(crate) fn recv_connection(
    mut connection: ResMut<MqttClient>,
    mut mqtt_events: EventWriter<MqttEvent>,
    mut error_events: EventWriter<MqttError>,
) {
    while let Ok(event) = connection.from_async_event.try_recv() {
        mqtt_events.send(MqttEvent(event));
    }

    while let Ok(err) = connection.from_async_error.try_recv() {
        error_events.send(MqttError(err));
    }
}
