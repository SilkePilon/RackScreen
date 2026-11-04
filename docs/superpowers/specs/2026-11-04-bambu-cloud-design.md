# Bambu Lab cloud login for the printer role

## Goal

The `printer` role works without LAN only mode and Developer Mode: the Pi
logs in to the user's Bambu Lab account and reads the printer's report from
Bambu's cloud MQTT broker. LAN mode stays as it is and stays the default.

## What Bambu's cloud does (unofficial, as reverse-engineered by
ha-bambulab, openHAB, Bambu-Lab-Cloud-API)

- Login: `POST https://api.bambulab.com/v1/user-service/user/login`
  `{"account", "password", "apiError": ""}`. The answer is one of:
  - `accessToken` non-empty: done.
  - `loginType: "verifyCode"`: an emailed code is needed. Ask for it with
    `POST .../user-service/user/sendemail/code {"email", "type": "codeLogin"}`,
    then `POST .../user/login {"account", "code"}`. HTTP 400 with
    `code: 1` is an expired code (a new one is sent), `code: 2` a wrong one.
  - `loginType: "tfa"` with `tfaKey`: an authenticator code is needed.
    `GET https://bambulab.com/api/csrf` (204, cookie `bbl_csrf_token`),
    then `POST https://bambulab.com/api/sign-in/tfa {"tfaKey", "tfaCode"}`
    with header `x-bbl-csrf-token` and the cookie; the token is in the
    `token` cookie of the answer.
- China accounts use `api.bambulab.cn` / `bambulab.cn` instead.
- The token is a JWT; its payload's `username` is the MQTT user `u_<uid>`.
  When it is not a JWT, `GET .../design-user-service/my/preference` gives
  `uid`. Tokens last about three months and cannot be refreshed.
- Printers: `GET .../iot-service/api/user/bind` (Bearer) gives
  `devices[] {dev_id, name, online, dev_product_name, dev_model_name}`.
- MQTT: `us.mqtt.bambulab.com:8883` (`cn.mqtt.bambulab.com` for China),
  TLS with a real certificate, user `u_<uid>`, password the token, topics
  as on the LAN (`device/<serial>/report`, `pushall` on `/request`).
- The printer must be bound to the account and not in LAN only mode.
- Logins can be blocked by Cloudflare (HTTP 403 with an HTML body); accounts
  made with Google or Apple sign-in have no password until one is set.

## Design

### Config (`bambu`)

- `mode: lan | cloud`, default `lan`.
- `region: global | china`, default `global`, cloud only.
- `serial` is used by both modes; in cloud mode the web UI picks it from the
  account's printer list.
- `host` and `access_code` only matter in LAN mode.
- `ready()`: enabled and serial set, plus host and access code in LAN mode.
  Cloud mode does not need the token to be "ready": the source waits for it,
  so a login takes effect without a restart.

### Token file

`$XDG_STATE_HOME/rackscreen/bambu.json` (next to `spotify.json`), mode 0600,
written with the same owned-file helper:
`{token, username, account, region, expires_at, error}`. The password is
never stored; it lives only for the login request. `error` is set when the
broker refuses the token, so the UI says "log in again".

### Source (`crates/sources/src/bambu.rs`, new `bambu_cloud.rs`)

- `BambuConfig` gains `tls: Tls::AnyCert | Tls::Verified` and a
  `cloud: Option<PathBuf>` token path. In cloud mode each session reads the
  token file first: none, or one marked refused, gives
  `PrinterStatus::LoggedOut` and a re-check every 10 s. A good one sets
  user/password for that session.
- Verified TLS uses `rustls-platform-verifier` (already in the tree through
  reqwest) with the ring provider.
- CONNACK refused (4/5) in cloud mode marks the token file refused and emits
  `LoggedOut`; the source then waits for a new login.
- `bambu_cloud.rs` holds the HTTP side as plain functions over a base URL so
  tests can point it at a local server: `login`, `submit_code`,
  `submit_tfa`, `printers`, `username_from_token`, token file read/write.

### Role

New `PrinterStatus::LoggedOut`: the tile says "Log in again". The role
gallery's "needs setup" check uses `ready()` as before.

### Web API (`crates/web/src/bambu.rs`)

- `GET /api/bambu` → `{mode, logged_in, account, expires_at, status}`.
- `POST /api/bambu/login {account, password, region}` → `{ok:true}` or
  `{needs:"code"}` / `{needs:"tfa"}`; the pending step (account, region,
  tfaKey) is kept in memory for 10 minutes.
- `POST /api/bambu/code {code}` → finishes the pending step, writes the
  token file.
- `POST /api/bambu/resend` → sends a new email code.
- `GET /api/bambu/printers` → `[{serial, name, model, online}]`.
- `POST /api/bambu/logout` → deletes the token file.
- Errors are short and human: wrong password, wrong or expired code,
  blocked by Bambu (try later), Google/Apple account without a password.

### Web UI (`web/src/pages/integrations/home.tsx`)

- A LAN / Bambu Cloud choice under the switch.
- LAN: host, serial, access code, as now.
- Cloud: region; when logged out, email + password + Log in, then a code
  field when asked; when logged in, "Logged in as x · expires in N days",
  Log out, and a printer picker that sets `bambu.serial`.

## Out of scope

Camera, print control, SMS login (China phone accounts), token refresh.

## Testing

Unit tests for the login answer parsing, JWT username, bind parsing, token
file round trip, config parse/validate per mode, the `LoggedOut` tile, and a
local HTTP server run through login → code → printers. The MQTT client is
already covered; cloud only changes the TLS verifier and the credentials.
