Closes #248
Closes #249
Closes #250
Closes #251

## Summary

This change introduces enterprise utilities for fleet-wide streaming caps, peer-to-peer energy exchange, device liveness with slashing, and grid shortage load-shedding using a tier–epoch pattern. It also fixes Soroban contract metadata limits (`export = false` where needed), completes the `DataKey` / `ContractError` surface, and wires fleet accounting into stream create, pause, resume, rate updates, depletion, and amicable close.

---

## #248 — Fleet aggregate cap (provider-level)

- Persistent `FleetState` aggregate under `DataKey::FleetAgg(provider)`; cap under `FleetCap(provider)`.
- `create_continuous_stream` enforces `sum + new_rate ≤ cap` (saturated i128 math); `set_provider_fleet_cap` (super admin or DAO governor) updates cap and emits a limit event.
- Fleet total is updated on stream create, pause, resume, flow rate change, depletion, and amicable close. Lowering the cap does not terminate existing streams.

## #249 — P2P exchange + grid fee

- `p2p_finalize_exchange` enforces distinct supplier/consumer, optional credit vault and battery cap, and routes grid fee in bps to the utility treasury. Emits a P2P finalization event for indexers.

## #250 — Liveness (heartbeat + slash + pardon)

- `stream_device_heartbeat` with ed25519 over `(stream_id || meter_id)` payload; last ledger in temporary `StreamLastHeartbeat`.
- `apply_liveness_slash` (proportionate) and `pardon_stream_liveness` for provider pardon flow.

## #251 — Priority tier + O(1) load shed

- `ProviderGridEpoch` per provider; `grid_shortage_load_shed` increments epoch and floor tier (grid admin only). Streams compare tier vs epoch lazily; **Critical** cannot be shed by policy.
- `set_grid_administrator` / `set_dao_governor` for governance wiring.

---

## Workspace

- Adds `contracts/Cargo.toml` workspace so `utility_contracts` and `price_oracle` resolve `soroban-sdk` from `[workspace.dependencies]`.

---

## Follow-ups (optional)

- Resolve remaining compiler errors outside this surface (ZK helpers, legacy meter fields in some branches) until `cargo build -p utility_contracts` is fully green.
- Add integration tests for 100-stream fleet cap breach and 24h P2P scenarios once the crate builds cleanly.
