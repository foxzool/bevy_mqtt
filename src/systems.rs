use std::time::Duration;

use bevy::prelude::NonSendMut;

use crate::client::MqttConnection;

pub(crate) fn recv_connection(mut connection: NonSendMut<MqttConnection>) {
    while let Ok(notification) = connection.recv_timeout(Duration::from_millis(1)) {
        println!("Received: {:?}", notification);
    }
}