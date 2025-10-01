# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.8.0] - 2025-01-01

### Changed
- **BREAKING**: Upgrade to Bevy 0.17.0
- Migrated from `EventWriter`/`EventReader` to `MessageWriter`/`MessageReader`
- Updated `add_event` calls to `add_message` in plugin setup
- Updated observer API from `Trigger<OnRemove, T>` to `On<Remove, T>`
- Fixed `TopicMessage` to implement `EntityEvent` with proper target handling
- Updated trigger API to use new closure-based approach
- All event types now implement both `Event` and `Message` traits for compatibility

### Technical Details
- Event/Message API overhaul for better entity targeting
- Observer pattern improvements for component lifecycle events
- Trigger system now requires function-based event construction
- Enhanced type safety for entity-specific events

## [0.7.1]

### Fixed
- Resolve crates.io publish issue with generated files
- Improve release notes generation with dynamic changelog

## [0.7.0]

### Changed
- **BREAKING**: Upgrade to Bevy 0.16.0
- Updated all Bevy dependencies to version 0.16
- Migration to new Bevy ECS patterns

## [0.6.0]

- bevy bump to 0.16

## [0.5.0-rc.1]

- bevy crates bump to 0.15.0-rc.1
- SubscribeTopic search mqtt client from ancestors
- Add `PackageCache` to store the package for each topic

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