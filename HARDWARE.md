# Hardware Spec Integration

This project does not connect to MQTT brokers directly from the smart contract. The contract is the settlement layer, while a backend service listens for device telemetry over MQTT, validates the readings, and then submits authenticated transactions to the contract.

## Architecture

```text
+---------+       +--------+       +---------+       +------------------+
|  Meter  | ----> |  MQTT  | ----> | Backend | ----> | Smart Contract   |
| Device  |       | Broker |       | Service |       | (Soroban/Stellar)|
+---------+       +--------+       +---------+       +------------------+
```

## Connection Flow

1. A smart meter or controller running on ESP32 or Raspberry Pi measures energy or utility consumption.
2. The device publishes usage data to an MQTT topic such as `meters/{meter_id}/usage`.
3. The backend service subscribes to the MQTT broker, parses the payload, and validates the device identity and reading format.
4. The backend maps the hardware meter to an on-chain `meter_id`.
5. The backend submits a signed contract call such as `deduct_units` or `update_usage`.
6. The contract updates stored meter data, transfers value when required, and emits events for downstream monitoring.

## MQTT Payload Example

```json
{
  "meter_id": 1,
  "timestamp": 1710000000,
  "watt_hours_consumed": 250,
  "units_consumed": 1
}
```

## Supported Hardware Devices

- ESP32 development boards with Wi-Fi and MQTT client support
- ESP32-S3 boards for higher-performance edge processing
- Raspberry Pi Zero 2 W for lightweight gateway deployments
- Raspberry Pi 4 for local aggregation and MQTT forwarding
- Raspberry Pi 5 for higher-throughput backend or gateway workloads

## Suggested Responsibilities

- Meter device: sample consumption data and publish telemetry
- MQTT broker: route messages reliably between devices and backend services
- Backend service: authenticate devices, validate readings, and call the contract
- Smart contract: store meter state, handle balances, and enforce settlement rules

## Notes

- The smart contract should not hold MQTT credentials or broker connection logic.
- MQTT authentication, TLS, retry handling, and device registry checks should live in the backend layer.
- If an ESP32 is too resource-constrained for direct blockchain signing, it should publish to MQTT and let the backend submit the on-chain transaction.
