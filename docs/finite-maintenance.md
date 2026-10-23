# Finite owner-local maintenance lifecycle

The environment core owns the product's desired-state and reconciliation ledger.
Version 0.3.0 adds finite maintenance consent and outcome reconciliation to that
same `Ledger` implementation. Operator file intake, actual host identification
and commands belong to `environment-operations`; private environments belong to
`environment-configurations`. No new coordination service or database owner is
introduced.

This is a local operator consent journal. It does not authenticate remote users,
issue credentials, sign Crowsi grants, start processes or call providers. Crowsi
continues to own shared security policy, credential custody and privileged
enforcement. Its Bridge actions are closed; maintenance must not be mislabeled
as `observe-provider` or `run-diagnostic`. A future real adapter needs separately
accepted operation-specific authorization, target CAS/fencing, cancellation,
bounded subprocess/network execution and fresh authenticated provider readback.
Every returned status/reservation explicitly has `execution_authorized: false`.
The separate npm owner and deployment Environment approvals are not replaced.

## Local state boundary

Maintenance methods require `Ledger::open_private` with an existing physical
absolute 0700 parent directory belonging to the effective Unix UID. DB and
existing SQLite sidecars must be physical 0600 single-link files of that owner.
No directories are created and no existing permission is repaired. SQLite opens
with NOFOLLOW, a finite busy timeout and FULL synchronous commits. Safe rustix
only supplies effective UID; SQLite and serde/digest implementations are reused.
The existing reconciliation `Ledger::open` contract remains available but cannot
use maintenance APIs. Non-Unix private maintenance is explicitly unsupported.

The SQLite file and its private directory must remain on reliable local storage.
This does not isolate another process with the same OS account, prevent an
operator copying/restoring/deleting their journal, or prove global exactly-once
effects. Do not reset the journal to retry unknown work. Only an accepted host
enforcer can establish the external execution boundary.

## Immutable finite plan

`vpremises://maintenance/finite-plan/v1` binds the environment, plan ID, local
owner and host, source/artifact/inventory/resolved-configuration SHA256, validity and all ordered steps.
The product computes the digest from the closed typed structure. Unknown fields
are refused. Approval lasts at most one hour, the complete batch has at most 300
seconds of dispatch budget, and at most 16 uniquely named steps. A step names a
product operation and the hashes of its before/after states; it contains no shell
command, arguments, executable path, token or arbitrary provider grant. Equal
before/after hashes are ambiguous for mutation readback and are refused. Ordinary
read-only planning/verification does not need a fabricated mutation reservation.

The host adapter must compute the observed context rather than accept claimed
owner/host/source metadata from an untrusted request. Approval IDs cannot be
reused. Source, artifact, inventory, host and owner must still match at reservation.
The fresh precondition must match the step's before-state and be observed within
ten seconds. It is not a fence: the real adapter must also reject a changed
resource at the actual effect boundary.

## At-most-one dispatch reservation, explicit unknown outcomes

1. `approve_maintenance` persists consent to the exact plan and context.
2. `reserve_maintenance` uses an immediate transaction to consume the next step
   and persist **unknown** before returning. The first reservation starts the
   overall batch budget. Two connections cannot reserve the same step. Crashing
   before or after a hypothetical dispatch never makes that step callable again.
3. An unknown step blocks other approvals/reservations for the same host and
   environment, including renamed IDs. Other hosts/environments are independent.
4. `reconcile_maintenance` accepts only the exact stored reservation and a bound
   independently obtained observation with a valid timestamp and evidence hash.
   An after-state observation advances the cursor; a before-state observation
   terminates as failed without retry; any third state remains unknown.
   Matching desired bytes proves observation, not that this process caused them.
5. Each resolved step retains its own receipt; duplicate/substituted receipts
   cannot advance the batch. No step beyond the finite list is admitted.
6. Stop/revoke prevents remaining reservations. It cannot undo or cancel an
   already in-flight external effect. Unknown remains unknown and is still
   readable/reconcilable after revocation or expiry. Clock rollback is rejected
   using a persistent high-water mark; the CLI must supply its actual clock.

The journal permits at most 4096 approved plans, each at most 16 KiB, with at most
16 fixed-size final receipts per plan and one bounded current observation. It
refuses growth at capacity; it does not implicitly prune consent/evidence.
There is no timer enablement, automatic retry, new signing authority or live
credential enrollment. Unit/integration fixtures exercise only disposable local
state; successful synthetic reconciliation is not production execution evidence.
