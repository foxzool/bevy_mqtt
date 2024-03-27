use std::time::Duration;

use bevy::log::trace;
use bevy::prelude::{ EventWriter, NonSendMut};
use rumqttc::{ConnectionError, Event};

use crate::client::MqttConnection;
use crate::events::{MqttError, MqttEvent};

pub(crate) fn recv_connection(mut connection: NonSendMut<MqttConnection>, mut mqtt_events: EventWriter<MqttEvent>, mut error_events: EventWriter<MqttError>) {
    while let Ok(notification) = connection.recv_timeout(Duration::from_millis(1)) {
        trace!("Received: {:?}", notification);
        match notification {
            Ok(event) => {
                mqtt_events.send(MqttEvent(event));
            }
            Err(err) => {
                error_events.send(MqttError(err));
            }
        }
    }
}