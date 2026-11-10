# Per-role screen times and role skipping

Brainstormed 2026-11-10. First of three specs (then the ISS globe, then the SEN live role).

## Goal

Replace "one `cycle_secs` per panel" with "seconds per role entry", and let a
role say it cannot show right now so the panel skips it (needed by the SEN
live role, which is only shown while the stream has live Earth footage).

## Config

A panel's `roles:` entry is either a bare name or a map:

```yaml
screens:
  - roles: [cpu, { role: iss, secs: 30 }, { role: sen, secs: 60 }]
    cycle_secs: 10        # default for entries without secs
```

- Bare entries keep working and use the panel's `cycle_secs` (default 15).
- `secs` range 3..=300, validated with the 1-based screen index and role name
  in the error, like `cycle_secs`.
- Serialisation writes a bare name when the entry has no own `secs`, so
  existing configs round-trip unchanged.

## Scheduler (`core::screens::ScreenState`)

- Holds `Vec<Entry { role, secs }>` instead of `Vec<Role>` + one `cycle_secs`.
  `cycle_secs()` stays as "the current entry's dwell" for callers (progress,
  web status).
- `can_show(role) -> bool` comes from the model (`Model::role_available`).
  Today only `Role::Sen` can be unavailable; every other role returns `true`.
- When a transition starts, the target is chosen then: the next entry after
  the current one whose role can show. It is stored for the transition
  (`Transition::to` reads it), so it cannot change mid-transition.
- If no other entry can show, no transition starts and the dwell restarts
  (the current role keeps showing; the SEN role then draws its fallback, the
  ISS globe).
- `request_leave(now)`: the current role asks to go now (SEN went non-live).
  Implemented with `make_due(now, 0)` so the serialising transition queue
  still applies.
- Shows, `jump_to`, `extend_dwell`, offsets and transition serialisation keep
  their behaviour; `make_due`/`extend_dwell`/`progress` use the current
  entry's secs.
- Runloop: each tick, for a panel whose current role is unavailable while
  another entry can show, call `request_leave`.

## Look-ahead for warm-up

`ScreenState::upcoming(now) -> Option<(Role, Secs)>`: the role the next
transition would go to if it started now, and the seconds until the dwell ends.
The runloop uses it to start the SEN stream about 8 s before its slot.

## Web UI and API

- `GET/PUT` config: a role entry is `string | { role, secs }` in
  `web/src/lib/types.ts`.
- Screens tab: each role chip gets a small seconds field ("15 s", empty =
  panel default); the cycle timeline draws each block with its own length;
  `/api/status` reports the current entry's dwell so the cycle ring stays
  right.
- Panel default (`cycle_secs`) stays editable as "Default time".

## Tests

- config: mixed bare/map entries load, validate (range), round-trip.
- screens: per-entry dwell; skip of unavailable role; all-unavailable keeps
  current; `request_leave`; `upcoming`.
- web: typecheck + existing tests.
