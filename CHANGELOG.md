# Changelog

All notable changes to FastCord are recorded here. Release bodies on GitHub
Releases reuse the section of the released version (`release.yml`).

## [Unreleased]

- Workspace scaffold with six crates (`fastcord-app`, `fastcord-discord`,
  `fastcord-voice`, `fastcord-media`, `fastcord-storage`, `fastcord-platform`).
- `fastcord-discord` gateway core: Snowflake ids, token-shape login validation,
  gateway frames/events, resume backoff, READY two-pass join with synthetic
  fixtures (to be replaced by the real capture in issue #45).
