# docs: DAO emergency runbook — circuit breaker, Wasm upgrade & state migration

## Summary

Adds `EMERGENCY_RUNBOOK.md` — a comprehensive, actionable emergency operations guide for the Utility Drip DAO covering every worst-case failure scenario with exact CLI commands.

## Changes

- `EMERGENCY_RUNBOOK.md` — new file (1,217 lines)

## What's included

| Section | Coverage |
|---|---|
| Roles & Responsibilities | DAO Admin, Compliance Officer, Finance Wallets, Oracle, Provider |
| Pre-Incident Checklist | Environment verification before any emergency action |
| Scenario A — Active Exploit | `challenge_service`, `emergency_shutdown`, velocity override revocation, cancel pending withdrawals |
| Scenario B — Protocol Pause | Per-meter and per-stream pause/resume, global velocity limiting |
| Scenario C — Wasm Hash Upgrade | Build → upload → propose → veto window → finalize → verify → rollback |
| Scenario D — State Migration | Pause → dump → deploy migration contract → migrate → diff verify → transfer balances |
| Scenario E — Multi-Sig Freeze | Cancel request, revoke approval, reconfigure after wallet compromise |
| Scenario F — Legal Freeze | Freeze meter, verify, release with council multi-sig, rotate compliance officer |
| Scenario G — Gas Buffer Exhaustion | Check balance, top up, initialize, withdraw excess |
| Scenario H — Admin Key Compromise | Initiate transfer, DAO veto window, execute, rotate dependent keys |
| Scenario I — Oracle Failure | Diagnose, update oracle address, resolve downstream challenges |
| Scenario J — Velocity Limit Breach | Apply override, tighten limits, revoke override |
| Post-Incident Procedures | Evidence preservation, challenge resolution, key rotation, 72-hour post-mortem |
| Multi-Sig Signer Reference Card | Standalone guide for Finance Wallet holders — full lifecycle + pre-approval checklist |
| Contact Tree | P1–P4 escalation matrix with response time targets |

## Acceptance criteria

- [x] Actionable, step-by-step emergency procedures with exact `stellar contract invoke` commands
- [x] Multi-sig signers have a clear understanding of their technical duties (Section 14 — standalone reference card)
- [x] Covers all worst-case failure scenarios including admin key compromise, oracle failure, flash drain, and state migration

## Labels

`documentation` `security` `devops`

## Reviewers

Assign: DAO Admin, at least one Finance Wallet holder, Security Lead
