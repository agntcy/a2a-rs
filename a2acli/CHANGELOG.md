# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/a2aproject/a2a-rs/releases/tag/a2a-cli-v0.1.0) - 2026-04-14

### Added

- migrate A2A crates to a2a-lf namespace ([#41](https://github.com/a2aproject/a2a-rs/pull/41))
- *(a2a-client)* make A2AClient generic over Transport to enable zero-cost static dispatch ([#43](https://github.com/a2aproject/a2a-rs/pull/43))
- add standalone a2acli CLI ([#38](https://github.com/a2aproject/a2a-rs/pull/38))

### Added

- add standalone `a2acli` binary crate
- add task push notification config CRUD commands
- make the package installable as `a2a-cli`