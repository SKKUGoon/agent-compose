# AGENTS.md

This repository implements `agent-compose` v2 in Rust.

## Hard Rules

- v2 only; no backward compatibility with `defaults`.
- Use top-level `provider` (singular) for model backend configuration.
- API key values must support env interpolation syntax in YAML.
- Each task must define exactly one executor: `agent`, `agents`, or `step`.
- `step` is Rust-only.
- `python_step` is invalid.

## v2 Schema Contract Checklist

### Top-level

- [ ] `version` exists and equals `"2"`.
- [ ] `name` is a non-empty string.
- [ ] `provider` exists.
- [ ] `runtime` exists.
- [ ] `schema.file` exists.
- [ ] `agents` exists and is non-empty.
- [ ] `tasks` exists and is non-empty.
- [ ] `output.from` exists.

### Provider

- [ ] `provider.kind` in `openai | anthropic | ollama`.
- [ ] `provider.api_key` is non-empty after env interpolation.
- [ ] `provider.default_model` optional string.
- [ ] `provider.base_url` optional string.

### Env Interpolation

- [ ] `${env:VAR}` resolves from environment.
- [ ] `${env:VAR:-fallback}` resolves to fallback if unset.
- [ ] Missing env var without fallback fails clearly.

### Runtime

- [ ] `runtime.context_mode` in `merged_and_refs | refs_only | merged_only`.
- [ ] `runtime.skip_policy` in `none | gatekeeper_controlled`.
- [ ] If `gatekeeper_controlled`, `runtime.gatekeeper` has:
  - [ ] `task`
  - [ ] `field`
  - [ ] `skip_tasks`

### Agents

- [ ] `agents.<id>.instructions` non-empty.
- [ ] `agents.<id>.input_model` references existing schema model.
- [ ] `agents.<id>.output_model` references existing schema model.
- [ ] `agents.<id>.model` optional override.

### Tasks

- [ ] `tasks.<id>.needs` references only known tasks.
- [ ] Exactly one executor present: `agent | agents | step`.
- [ ] `tasks.<id>.agent` references known agent.
- [ ] `tasks.<id>.agents` is non-empty and each is known.
- [ ] `tasks.<id>.step` is known Rust step.
- [ ] `tasks.<id>.input` resolves to object.

### External Schema File

- [ ] `schema.file` path resolves relative to config path (unless absolute).
- [ ] External file contains top-level `models`.
- [ ] `schema.models` loaded from external file before validation.

### Schema Models

- [ ] Model `type` is `object`.
- [ ] `fields` is an object.
- [ ] Supported field forms:
  - [ ] primitive `type`: `string | boolean | integer | number`
  - [ ] `type: array` with `items`
  - [ ] `$ref`
- [ ] `required`, `nullable`, `default`, `enum`, and length constraints validated.
- [ ] `$ref` targets existing schema model.

### DAG + Execution

- [ ] Task graph is acyclic.
- [ ] Topological execution order is valid.
- [ ] `agents: [...]` executes in parallel and merged output is available.
- [ ] Gatekeeper skip policy marks tasks as skipped with safe defaults.

### Ref Resolution

- [ ] Full ref `${{ path.to.value }}` returns native JSON.
- [ ] Inline refs in strings are substituted as strings.
- [ ] Unknown path raises explicit error.

### Steps

- [ ] `step` dispatch runs in Rust only.
- [ ] `build_pipeline_output` exists.
- [ ] Unknown steps fail clearly.

## Build Check

- Required verification command: `cargo build`.
