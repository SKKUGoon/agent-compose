# AGENTS.md

This repository implements `agent-compose` v2 in Rust.

## Hard Rules

- v2 only; no backward compatibility with `defaults`.
- Multi-chain is first-class. Configuration is top-level metadata plus a non-empty `chains` map.
- API key values must support env interpolation syntax in YAML.
- Each task must define exactly one executor: `agent`, `agents`, or `step`.
- `step` is Rust-only.
- `python_step` is invalid.

## v2 Schema Contract Checklist

### Top-level

- [ ] `version` exists and equals `"2"`.
- [ ] `name` is a non-empty string.
- [ ] `schema.file` exists.
- [ ] `chains` exists and is non-empty.

### Chains

- [ ] `chains.<id>` key is non-empty.
- [ ] `chains.<id>.provider` exists.
- [ ] `chains.<id>.runtime` exists.
- [ ] `chains.<id>.agents` exists and is non-empty.
- [ ] `chains.<id>.tasks` exists and is non-empty.
- [ ] `chains.<id>.output.from` exists.
- [ ] `chains.<id>.serve.host` non-empty.
- [ ] `chains.<id>.serve.port` valid `u16`.
- [ ] `(serve.host, serve.port)` pair is unique across chains.

### Provider

- [ ] `chains.<id>.provider.kind` in `openai | anthropic | ollama`.
- [ ] `chains.<id>.provider.api_key` is non-empty after env interpolation.
- [ ] `chains.<id>.provider.default_model` optional string.
- [ ] `chains.<id>.provider.base_url` optional string.

### Env Interpolation

- [ ] `${env:VAR}` resolves from environment.
- [ ] `${env:VAR:-fallback}` resolves to fallback if unset.
- [ ] Missing env var without fallback fails clearly.

### Runtime

- [ ] `chains.<id>.runtime.context_mode` in `merged_and_refs | refs_only | merged_only`.
- [ ] `chains.<id>.runtime.skip_policy` in `none | gatekeeper_controlled`.
- [ ] If `gatekeeper_controlled`, `chains.<id>.runtime.gatekeeper` has:
  - [ ] `task`
  - [ ] `field`
  - [ ] `skip_tasks`

### Agents

- [ ] `chains.<id>.agents.<id>.instructions` non-empty.
- [ ] `chains.<id>.agents.<id>.input_model` references existing schema model.
- [ ] `chains.<id>.agents.<id>.output_model` references existing schema model.
- [ ] `chains.<id>.agents.<id>.model` optional override.

### Tasks

- [ ] `chains.<id>.tasks.<id>.needs` references only known tasks.
- [ ] Exactly one executor present: `agent | agents | step`.
- [ ] `chains.<id>.tasks.<id>.agent` references known agent.
- [ ] `chains.<id>.tasks.<id>.agents` is non-empty and each is known.
- [ ] `chains.<id>.tasks.<id>.step` is known Rust step.
- [ ] `chains.<id>.tasks.<id>.input` resolves to object.

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
