# Channel Bridge JavaScript Example

This example shows the current public reference-first managed bridge package lane.

## Validate

```bash
loong plugins doctor --root "examples/plugins-process/channel-bridge-javascript" --profile sdk-release
```

## Inspect

```bash
loong plugins inventory --root "examples/plugins-process/channel-bridge-javascript"
```

## What it proves

- `channel_id=weixin`
- `setup.surface=channel`
- `transport_family=wechat_clawbot_ilink_bridge`
- `target_contract=weixin:<account>:contact:<id> | weixin:<account>:room:<id>`
- `channel_runtime_contract=loong_channel_bridge_v1`
- `channel_runtime_operations_json`
- `channel_runtime_operation_specs_json`
