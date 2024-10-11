# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

- SubscribeTopic search mqtt client from ancestors

## [0.4.2] - 2024-09-26

- MqttPublishPacket contains entity

## [0.4.1] - 2024-09-26

- subscribe many topics same time

## [0.4.0] - 2024-09-26

- every MQTT client is a component now
- can spawn a `SubscribeTopic` component to subscribe to a topic and observe the messages

## [0.3.3] - 2024-09-24

- Update MQTT example and library handling: Refactor error handling, subscribe method, and event types for clarity and
  robustness

## [0.3.2] - 2024-09-13

- Improve MQTT subscription handling and add LogPlugin to example

## [0.3.1] - 2024-09-12

- refactor code
- auto reconnect mqtt client

## [0.2.0] - 2024-07-05

- bump bevy version to `0.14.*`

## [0.1.1] - 2024-03-29

- use tokio runtime for websocket

## [0.1.0] - 2024-03-27

- Initial release