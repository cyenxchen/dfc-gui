# CLAUDE.md

Shared repository instructions for coding agents working in this project.
`AGENTS.md` is symlinked to this file, so both entry points should follow the same rules.

## Release Rules

- Before creating or pushing any git tag, update `Cargo.toml`'s package `version` so it matches the release version.
- Commit the version bump before creating the tag.
- Do not create or push a tag if the tag version and `Cargo.toml` version are out of sync.
