# agent-compose

[![agent-compose](https://github.com/SKKUGoon/agent-compose/actions/workflows/agent-compose.yml/badge.svg)](https://github.com/SKKUGoon/agent-compose/actions/workflows/agent-compose.yml)
![Rust](https://img.shields.io/badge/rust-2024%20edition-orange)
![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)
![Tag](https://img.shields.io/badge/tag-v0.1.0-blue)

`agent-compose` is a Rust CLI for running multi-step AI pipelines from YAML. You define a chain once, then run it locally, expose it as HTTP/MCP, or call it from other tools.

## Quick start

```bash
git clone https://github.com/SKKUGoon/agent-compose.git
cd agent-compose
cp .env.example .env
# set OPENAI_API_KEY in .env
cargo build
```

Example build output:

```text
$ cargo build
   Compiling agent-compose v0.1.0 (.../agent-compose)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.42s
```

## Run a chain (interactive/plain mode)

```bash
cargo run -- run --config agent-compose.yaml --chain news_cleaning --plain
```

Example session:

```text
$ cargo run -- run --config agent-compose.yaml --chain news_cleaning --plain
agent-compose interactive mode. Type /quit to exit.
Commands: /json on|off
You> Oil shipments were delayed near the Red Sea after military activity.
Agent> gatekeeper=pass reason=Geopolitical conflict with market relevance
Agent> Regional military activity disrupted shipping lanes and raised commodity risk.
You> /quit
```

## Serve and call

Start servers for all chains in `agent-compose.yaml`:

```bash
cargo run -- serve start --config agent-compose.yaml
```

Check process table:

```bash
cargo run -- ps --config agent-compose.yaml
```

Example output:

```text
$ cargo run -- ps --config agent-compose.yaml
CHAIN         STATUS   ENDPOINT
news_cleaning Running  127.0.0.1:8787
test          Running  127.0.0.1:8788
```

Call a running chain:

```bash
cargo run -- call "Turkey's central bank signaled tighter policy after inflation surprise." \
  --config agent-compose.yaml \
  --chain news_cleaning \
  --json
```

Example output (shortened):

```json
{
  "ok": true,
  "result": {
    "passed_gatekeeper": true,
    "gatekeeper_reason": "Macro policy signal with financial relevance",
    "summary_distilled": "Turkey's central bank...",
    "countries": [
      {
        "country": "Turkey",
        "note": "Directly referenced"
      }
    ]
  },
  "error": null
}
```

## `agent-compose.yaml` (what you edit most)

This file is your workflow control center.

- `version: "2"`: uses the v2 engine.
- `chains`: each chain is an independently runnable pipeline.
- `provider`: model vendor settings and API key via env interpolation (for example `${env:OPENAI_API_KEY}`).
- `runtime`: execution behavior (`context_mode`, retries, and optional gatekeeper skip policy).
- `agents`: each agent has instructions plus strict `input_model` and `output_model`.
- `tasks`: DAG execution graph.
  - Single-agent task: `agent: ...`
  - Parallel multi-agent task: `agents: [...]`
  - Rust step task: `step: build_pipeline_output`
- `output.from`: final value path returned by the chain.

In this repo, `news_cleaning` runs this flow:

`perceive -> gate -> classify (parallel country/asset/policy/event) -> city -> finalize`

`gatekeeper_controlled` skip policy is enabled, so selected tasks can be skipped when `passed_gatekeeper` is false.

## `datamodels.yaml` (your input/output contract)

This file defines every model used by agents and tasks.

- Models are strongly typed objects.
- Fields support primitive types, arrays, enums, defaults, nullability, and `$ref`.
- Agent IO is validated against these models, so outputs stay predictable.

In this repo, important models include:

- `PerceptionInput` / `PerceptionOutput`
- `GatekeeperInput` / `GatekeeperOutput`
- `CountryOutput`, `AssetOutput`, `PolicyOutput`, `EventOutput`, `CityOutput`
- `PipelineOutput` (final response contract)

If you want to change final API shape, start with `PipelineOutput` in `datamodels.yaml` and keep tasks aligned with it.

## CLI usage screenshots

Place screenshots in `./images` using these names:

![CLI run screenshot](./images/cli-run.png)
![Serve status screenshot](./images/serve-status.png)

## MCP spec command

```bash
cargo run -- mcp_spec --config agent-compose.yaml --all --pretty
```

Example output (shortened):

```json
{
  "servers": [
    {
      "name": "news_cleaning",
      "transport": "http",
      "server_url": "http://127.0.0.1:8787/rpc",
      "tools": [
        {
          "name": "infer",
          "description": "Run agent-compose chain inference"
        }
      ]
    }
  ]
}
```

## Version tags (CI trigger)

The GitHub Actions workflow runs only for tags in exact `vX.Y.Z` format.

```bash
git tag v1.2.3
git push origin v1.2.3
```

## License

Apache-2.0. See `LICENSE`.

## Real Usage

### 1. Freelance University Data Analysis Project

```yaml
# datamodels.yaml
models:
  DrugInput:
    type: object
    fields:
      drug_name:
        type: string
        required: true
      drug_code:
        type: string
        required: true

  WeightOutput:
    type: object
    fields:
      weight:
        type: number
        required: true
      unit:
        type: string
        required: true

  VolumeOutput:
    type: object
    fields:
      volume:
        type: number
        required: true
      unit:
        type: string
        required: true

  DrugExtractionOutput:
    type: object
    fields:
      drug_name:
        type: string
        required: true
      drug_code:
        type: string
        required: true
      weight:
        type: number
        required: true
      weight_unit:
        type: string
        required: true
      volume:
        type: number
        required: true
      volume_unit:
        type: string
        required: true
```

```yaml
# agent-compose.yaml
version: "2"
name: data-cleaning

schema:
  file: datamodels.yaml

chains:  
  healthEcon:
    provider:
      kind: openai
      api_key: ${env:OPENAI_API_KEY}
      default_model: gpt-5-nano
    
    runtime:
      context_mode: merged_and_refs
      skip_policy: none
      retry:
        contract_max_attempts: 2
        contract_backoff_ms: 300

    serve:
      host: 127.0.0.1
      port: 8789
      description: Health and economic analysis pipeline

    agents:
      weight_agent:
        instructions: |
          You are inspecting the drug name. You are extracting the weight of the drug and the unit of the weight.
          Units are e.g. mg, g, µg.

          If the drug name does not contain a weight, return -1 for the weight and "unknown" for the unit.
          Return ONLY WeightOutput
        input_model: DrugInput
        output_model: WeightOutput

      volume_agent:
        instructions: |
          You are inspecting the drug name. You are extracting the volume of the drug and the unit of the volume.
          Units are e.g. ml, l, µl, 병

          If the drug name does not contain a volume, return -1 for the volume and "unknown" for the unit.
          Return ONLY VolumeOutput
        input_model: DrugInput
        output_model: VolumeOutput

    tasks:
      extraction:
        agents: [weight_agent, volume_agent]
        input:
          drug_name: ${{ input.drug_name }}
          drug_code: ${{ input.drug_code }}
      
      finalize:
        needs: [extraction]
        step: build_pipeline_output
        input:
          drug_name: ${{ input.drug_name }}
          drug_code: ${{ input.drug_code }}
          weight: ${{ tasks.extraction.weight_agent.weight }}
          weight_unit: ${{ tasks.extraction.weight_agent.unit }}
          volume: ${{ tasks.extraction.volume_agent.volume }}
          volume_unit: ${{ tasks.extraction.volume_agent.unit }}

    output:
      from: tasks.finalize.output
      model: DrugExtractionOutput

```
