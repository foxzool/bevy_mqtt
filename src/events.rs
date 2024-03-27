use bevy::prelude::{Deref, DerefMut, Event};

/// A wrapper around rumqttc::Event
#[derive(Debug, Clone, PartialEq, Eq, Deref, DerefMut, Event)]
pub struct MqttEvent(pub rumqttc::Event);

/// A wrapper around rumqttc::ConnectionError
#[derive(Debug, Deref, DerefMut, Event)]
pub struct MqttError(pub rumqttc::ConnectionError);
