use bevy::prelude::{Deref, DerefMut, Resource};

/// A struct that represents the settings for an MQTT connection
#[derive(Resource)]
pub struct MqttSetting {
    /// Options to configure the behaviour of MQTT connection
    pub mqtt_options: rumqttc::MqttOptions,
    /// specifies the capacity of the bounded async channel.
    pub cap: usize,
}

/// A component that represents an MQTT client
#[derive(Resource, Deref, DerefMut)]
pub struct MqttClient(pub rumqttc::Client);

/// A component that represents an MQTT connection
#[derive(Deref, DerefMut)]
pub struct MqttConnection(pub rumqttc::Connection);