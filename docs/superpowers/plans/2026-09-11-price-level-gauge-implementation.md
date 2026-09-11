# Price Level Gauge Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the `price` role's 48-stub ring with a five-band level gauge (needle, big euro price, one-word verdict) fed by Energy-Charts quarter-hour prices and a three-day rolling average.

**Architecture:** `rackscreen-core` gets a `Drawable::Text` primitive, pure level maths (`price_level`) and the new `price_scene`; the model stores 96 euro slots plus the average and eases a needle `Smooth`. `rackscreen-sources::prices` becomes a single Energy-Charts poller (parser, quarter-hour bucketing, three-day mean). `rackscreen-app` config and wiring, the setup TUI fields and the README follow.

**Tech Stack:** Rust 1.85, chrono / chrono-tz, serde_json, reqwest (existing `client()`), fontdue via the existing text module, ratatui setup TUI.

Spec: `docs/superpowers/specs/2026-09-11-price-level-gauge-design.md`.

## Global Constraints

- All commands run from the repo root `/home/silke/Documents/GitHub/RackScreen`.
- `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings` must pass before every commit.
- The `sim` feature is needed for the simulator and gifs example: `cargo run --example gifs -p rackscreen-app --features sim`.
- Band words are exactly `V.CHEAP`, `CHEAP`, `NORMAL`, `PRICEY`, `V.PRICEY`; the no-data badge is `no data`; the caption is `€/kWh`.
- Ratio edges are `0.30, 0.60, 0.90, 1.15, 1.40, 1.70`.
- Energy-Charts URL is `https://api.energy-charts.info/v2/price`; `poll_secs` floor is 900; `vat_pct` default 21.
- Price events carry `eur_per_kwh: Vec<f32>` with exactly 96 entries (NaN for unknown slots) and `avg_eur_per_kwh: f32`.
- Commit messages end with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.

## File map

| File | Change |
|---|---|
| `crates/core/src/scene.rs` | `Drawable::Text` variant; `Role::Price` dispatch drops the hour argument |
| `crates/render/src/renderer.rs` | draws `Drawable::Text` |
| `crates/core/src/electricity.rs` | `PriceLevel`, `LEVEL_EDGES`, `price_level()`, `energy_charts_zone_for()` |
| `crates/core/src/event.rs` | `Event::Prices` new fields |
| `crates/core/src/model.rs` | `PriceState` in euro, `local_slot`, `price_needle: Smooth`, `current_price()`, `smooth_price_needle()` |
| `crates/core/src/scene_electricity.rs` | new `price_scene`, old ring removed, tests rewritten |
| `crates/app/src/runloop.rs` | `set_local_slot` from the wall clock |
| `crates/app/examples/gifs.rs` | `SLOT` instead of `HOUR` |
| `crates/render/tests/golden.rs` | price model in euro slots; golden `price.png` regenerated |
| `crates/sources/src/prices.rs` | Energy-Charts only: parser, `slots_for_day`, `mean_over_days`, `PriceConfig`, `run_prices` |
| `crates/sources/tests/fixtures/energy-charts-price.json` | new fixture (already saved); `energyzero.json`, `entsoe.xml`, `entsoe-a03.xml` deleted |
| `crates/sources/Cargo.toml` | drop `quick-xml` |
| `crates/sources/src/fake.rs` | 96-slot euro curve plus average |
| `crates/app/src/config.rs` | `PriceCfg { enabled, zone, vat_pct, poll_secs }` |
| `crates/app/src/run.rs` | `price_zone()` replaces `price_source()` |
| `crates/setup/src/screens/configure.rs` | fields `PriceEnabled`, `PriceZone`, `PriceVat`; `Choice` kind removed |
| `crates/setup/src/screens/screens.rs` | role hint `prices off` / `no price zone` |
| `README.md`, `config.example.yaml` | Energy-Charts, new fields, attribution |
| `.github/media/screens/price.gif` | regenerated |

---

### Task 1: `Drawable::Text` primitive

**Files:**
- Modify: `crates/core/src/scene.rs:69-80` (enum `Drawable`, after `Tick`)
- Modify: `crates/render/src/renderer.rs:179-192` (match arm after `Tick`)
- Test: `crates/render/src/renderer.rs` tests module

**Interfaces:**
- Produces: `Drawable::Text { cx: f32, cy: f32, text: String, px: f32, color: Color, alpha: f32 }`, centred on `(cx, cy)` like a badge's text.

- [ ] **Step 1: Write the failing renderer test**

Append inside `mod tests` in `crates/render/src/renderer.rs` (after the existing `Tick` test):

```rust
    #[test]
    fn text_draws_centred_glyphs() {
        use rackscreen_core::scene::{Drawable, Scene};
        use rackscreen_core::theme::layout::{CX, CY};
        use rackscreen_core::theme::WHITE;
        let mut r = Renderer::new().unwrap();
        let mut px = Pixmap::new(240, 240).unwrap();
        let mut s = Scene::new();
        s.push(Drawable::Text {
            cx: CX,
            cy: CY,
            text: "0.171".into(),
            px: 40.0,
            color: WHITE,
            alpha: 1.0,
        });
        r.render(&s, &mut px);
        let lit = (100..140)
            .flat_map(|y| (60..180).map(move |x| (x, y)))
            .filter(|&(x, y)| pixel(&px, x, y) != (0, 0, 0))
            .count();
        assert!(lit > 200, "glyph pixels in the centre band: {lit}");
        assert_eq!(pixel(&px, 120, 20), (0, 0, 0), "nothing above the text");
        assert_eq!(pixel(&px, 120, 220), (0, 0, 0), "nothing below the text");
    }
```

- [ ] **Step 2: Run it to see it fail**

Run: `cargo test -p rackscreen-render text_draws_centred_glyphs`
Expected: compile error `no variant named Text found for enum Drawable`.

- [ ] **Step 3: Add the variant**

In `crates/core/src/scene.rs`, after the `Tick { .. }` variant inside `pub enum Drawable`:

```rust
    /// Free-standing centred text: a badge's text without its box.
    Text {
        cx: f32,
        cy: f32,
        text: String,
        px: f32,
        color: Color,
        alpha: f32,
    },
```

- [ ] **Step 4: Draw it**

In `crates/render/src/renderer.rs`, after the `Drawable::Tick { .. } => { ... }` arm:

```rust
            Drawable::Text {
                cx,
                cy,
                text,
                px: text_px,
                color,
                alpha,
            } => {
                self.text
                    .draw_centered(px, text, *text_px, *cx, *cy, *color, *alpha);
            }
```

- [ ] **Step 5: Run the test and the workspace**

Run: `cargo test -p rackscreen-render text_draws_centred_glyphs`
Expected: `test tests::text_draws_centred_glyphs ... ok`

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: all green (no other code matches `Drawable` exhaustively).

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/scene.rs crates/render/src/renderer.rs
git commit -m "feat(render): Drawable::Text, centred text without a badge box

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 2: Level maths and the Energy-Charts zone table

**Files:**
- Modify: `crates/core/src/electricity.rs` (top of file, after the imports)
- Test: same file, `mod tests`

**Interfaces:**
- Produces: `pub enum PriceLevel { VeryCheap, Cheap, Normal, Pricey, VeryPricey }` with `ALL: [PriceLevel; 5]`, `word() -> &'static str`, `color() -> Color`, `index() -> usize`.
- Produces: `pub const LEVEL_EDGES: [f32; 6]`.
- Produces: `pub fn price_level(price: f32, avg: f32) -> (PriceLevel, f32)`; `f32` is the needle position `0..=1`.
- Produces: `pub fn energy_charts_zone_for(zone: &str) -> Option<&'static str>`.

- [ ] **Step 1: Write the failing tests**

Add to `mod tests` in `crates/core/src/electricity.rs`:

```rust
    #[test]
    fn price_level_bands_and_needle() {
        let avg = 0.20;
        let at = |ratio: f32| price_level(ratio * avg, avg);
        // band edges land on the fifths of the arc
        assert_eq!(at(0.60), (PriceLevel::Cheap, 0.2));
        assert_eq!(at(0.90), (PriceLevel::Normal, 0.4));
        assert_eq!(at(1.15), (PriceLevel::Pricey, 0.6));
        assert_eq!(at(1.40), (PriceLevel::VeryPricey, 0.8));
        // linear inside a band: the average sits 0.1 into the 0.25-wide NORMAL band
        let (lvl, t) = at(1.0);
        assert_eq!(lvl, PriceLevel::Normal);
        assert!((t - 0.48).abs() < 1e-5, "{t}");
        // clamped at both ends
        assert_eq!(at(0.30), (PriceLevel::VeryCheap, 0.0));
        assert_eq!(at(0.05), (PriceLevel::VeryCheap, 0.0));
        assert_eq!(at(1.70), (PriceLevel::VeryPricey, 1.0));
        assert_eq!(at(3.00), (PriceLevel::VeryPricey, 1.0));
        // a negative price against a positive average is as cheap as it gets
        assert_eq!(price_level(-0.006, avg), (PriceLevel::VeryCheap, 0.0));
        // no usable average: normal, mid-arc
        assert_eq!(price_level(0.1, 0.0), (PriceLevel::Normal, 0.5));
        assert_eq!(price_level(0.1, -0.2), (PriceLevel::Normal, 0.5));
        assert_eq!(price_level(0.1, f32::NAN), (PriceLevel::Normal, 0.5));
        assert_eq!(price_level(f32::NAN, avg), (PriceLevel::Normal, 0.5));
    }

    #[test]
    fn price_level_words_colours_and_order() {
        let words: Vec<&str> = PriceLevel::ALL.iter().map(|l| l.word()).collect();
        assert_eq!(
            words,
            ["V.CHEAP", "CHEAP", "NORMAL", "PRICEY", "V.PRICEY"]
        );
        for (i, l) in PriceLevel::ALL.iter().enumerate() {
            assert_eq!(l.index(), i);
        }
        assert_eq!(PriceLevel::VeryCheap.color(), GREEN);
        assert_eq!(PriceLevel::Normal.color(), AMBER);
        assert_eq!(PriceLevel::VeryPricey.color(), RED);
        assert_eq!(PriceLevel::Cheap.color(), GREEN.mix(AMBER, 0.5));
        assert_eq!(PriceLevel::Pricey.color(), AMBER.mix(RED, 0.5));
    }

    #[test]
    fn energy_charts_zones() {
        assert_eq!(energy_charts_zone_for("nl"), Some("NL"));
        assert_eq!(energy_charts_zone_for("DE"), Some("DE-LU"));
        assert_eq!(energy_charts_zone_for("DE-LU"), Some("DE-LU"));
        assert_eq!(energy_charts_zone_for("DK-DK1"), Some("DK1"));
        assert_eq!(energy_charts_zone_for("NO-NO5"), Some("NO5"));
        assert_eq!(energy_charts_zone_for("SE-SE3"), Some("SE3"));
        assert_eq!(energy_charts_zone_for("IT-NO"), Some("IT-North"));
        for z in ["BE", "FR", "AT", "CH", "ES", "PT", "PL", "FI", "CZ"] {
            assert_eq!(energy_charts_zone_for(z), Some(z), "{z}");
        }
        assert_eq!(energy_charts_zone_for("XX"), None);
        assert_eq!(energy_charts_zone_for(""), None);
    }
```

If the tests module has no `use super::*;` plus theme imports, add at its top:

```rust
    use super::*;
    use crate::theme::{AMBER, GREEN, RED};
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p rackscreen-core price_level`
Expected: compile errors, `cannot find type PriceLevel`.

- [ ] **Step 3: Implement**

In `crates/core/src/electricity.rs`, change the import line and add the block right after it:

```rust
use crate::theme::{Color, AMBER, GREEN, RED};

/// The five bands of the price level gauge, cheapest first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PriceLevel {
    VeryCheap,
    Cheap,
    Normal,
    Pricey,
    VeryPricey,
}

impl PriceLevel {
    pub const ALL: [PriceLevel; 5] = [
        PriceLevel::VeryCheap,
        PriceLevel::Cheap,
        PriceLevel::Normal,
        PriceLevel::Pricey,
        PriceLevel::VeryPricey,
    ];

    /// The badge word.
    pub fn word(self) -> &'static str {
        match self {
            PriceLevel::VeryCheap => "V.CHEAP",
            PriceLevel::Cheap => "CHEAP",
            PriceLevel::Normal => "NORMAL",
            PriceLevel::Pricey => "PRICEY",
            PriceLevel::VeryPricey => "V.PRICEY",
        }
    }

    /// The band colour, green through amber to red.
    pub fn color(self) -> Color {
        match self {
            PriceLevel::VeryCheap => GREEN,
            PriceLevel::Cheap => GREEN.mix(AMBER, 0.5),
            PriceLevel::Normal => AMBER,
            PriceLevel::Pricey => AMBER.mix(RED, 0.5),
            PriceLevel::VeryPricey => RED,
        }
    }

    /// 0 for the cheapest band, 4 for the dearest.
    pub fn index(self) -> usize {
        PriceLevel::ALL
            .iter()
            .position(|l| *l == self)
            .expect("every level is in ALL")
    }
}

/// Band edges as `price / average` ratios (Tibber's cut-offs), with the outer
/// clamps: below 0.60 is very cheap, above 1.40 very pricey.
pub const LEVEL_EDGES: [f32; 6] = [0.30, 0.60, 0.90, 1.15, 1.40, 1.70];

/// Where `price` sits against `avg`: the band, and the needle position in
/// `0..=1` that is linear inside each fifth of the arc. Ratios past the outer
/// edges clamp; an average that is not positive (or not finite), or a price
/// that is not finite, reads as `Normal` in the middle of the arc.
pub fn price_level(price: f32, avg: f32) -> (PriceLevel, f32) {
    if !(avg > 0.0) || !price.is_finite() {
        return (PriceLevel::Normal, 0.5);
    }
    let ratio = price / avg;
    let band = LEVEL_EDGES[1..5]
        .iter()
        .position(|edge| ratio < *edge)
        .unwrap_or(4);
    let (lo, hi) = (LEVEL_EDGES[band], LEVEL_EDGES[band + 1]);
    let inner = ((ratio - lo) / (hi - lo)).clamp(0.0, 1.0);
    (PriceLevel::ALL[band], (band as f32 + inner) / 5.0)
}

/// The Energy-Charts bidding zone an Electricity Maps zone maps to.
pub fn energy_charts_zone_for(zone: &str) -> Option<&'static str> {
    Some(match zone.trim().to_ascii_uppercase().as_str() {
        "NL" => "NL",
        "BE" => "BE",
        "FR" => "FR",
        "AT" => "AT",
        "CH" => "CH",
        "ES" => "ES",
        "PT" => "PT",
        "PL" => "PL",
        "FI" => "FI",
        "CZ" => "CZ",
        "DE" | "DE-LU" => "DE-LU",
        "DK-DK1" => "DK1",
        "DK-DK2" => "DK2",
        "NO-NO1" => "NO1",
        "NO-NO2" => "NO2",
        "NO-NO3" => "NO3",
        "NO-NO4" => "NO4",
        "NO-NO5" => "NO5",
        "SE-SE1" => "SE1",
        "SE-SE2" => "SE2",
        "SE-SE3" => "SE3",
        "SE-SE4" => "SE4",
        "IT-NO" => "IT-North",
        _ => return None,
    })
}
```

Note on `at(0.60)`: `0.60 * 0.20 / 0.20` is exactly `0.6` in f32 so `ratio < 0.60` is false and the band is `Cheap` with `inner = 0`. If clippy complains about `!(avg > 0.0)` (`neg_cmp_op_on_partial_ord`), keep it and add `#[allow(clippy::neg_cmp_op_on_partial_ord)]` on the function: the form is deliberate so NaN falls through.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p rackscreen-core electricity::`
Expected: the three new tests pass along with the existing partition tests.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/electricity.rs
git commit -m "feat(core): price level bands, needle maths and Energy-Charts zone table

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 3: Energy-Charts parser, quarter-hour bucketing and the three-day mean

**Files:**
- Modify: `crates/sources/src/prices.rs` (add functions; the old EnergyZero / ENTSO-E code stays until Task 4)
- Test: `crates/sources/src/prices.rs` `mod tests`
- Fixture: `crates/sources/tests/fixtures/energy-charts-price.json` (already in the tree, 288 entries for NL 2026-09-09..11)

**Interfaces:**
- Produces: `pub const SLOTS_PER_DAY: usize = 96;`
- Produces: `pub fn parse_energy_charts(json: &str) -> anyhow::Result<Vec<(DateTime<Utc>, f32)>>` (€/MWh samples).
- Produces: `pub fn eur_per_kwh(eur_per_mwh: f32, vat_pct: f32) -> f32`.
- Produces: `pub fn slots_for_day<Z: TimeZone>(samples: &[(DateTime<Utc>, f32)], day: NaiveDate, tz: &Z, vat_pct: f32) -> Vec<f32>` (96 entries, NaN gaps, €/kWh incl. VAT).
- Produces: `pub fn mean_over_days<Z: TimeZone>(samples: &[(DateTime<Utc>, f32)], days: &[NaiveDate], tz: &Z, vat_pct: f32) -> f32` (NaN when nothing matches).

- [ ] **Step 1: Write the failing tests**

Add to `mod tests` in `crates/sources/src/prices.rs`:

```rust
    const EC: &str = include_str!("../tests/fixtures/energy-charts-price.json");

    #[test]
    fn energy_charts_fixture_parses_three_days_of_quarter_hours() {
        let pts = parse_energy_charts(EC).unwrap();
        assert_eq!(pts.len(), 288, "3 days x 96 slots");
        // 2026-09-09T00:00+02:00 is 22:00Z the day before
        assert_eq!(pts[0].0, Utc.with_ymd_and_hms(2026, 9, 8, 22, 0, 0).unwrap());
        assert_eq!(pts[0].1, 150.31);
        assert_eq!(pts[287].1, 192.05);
        assert!(parse_energy_charts("{}").is_err(), "no data array");
        assert!(parse_energy_charts(r#"{"data":[]}"#).is_err(), "empty data");
        // a null price is skipped, not zero
        let one = parse_energy_charts(
            r#"{"data":[{"timestamp":"2026-09-11T10:00:00+02:00","values":{"day_ahead_price":null}},
                        {"timestamp":"2026-09-11T10:15:00+02:00","values":{"day_ahead_price":12.5}}]}"#,
        )
        .unwrap();
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].1, 12.5);
    }

    #[test]
    fn energy_charts_slots_are_local_quarter_hours_in_euro_with_vat() {
        let pts = parse_energy_charts(EC).unwrap();
        let day = NaiveDate::from_ymd_opt(2026, 9, 11).unwrap();
        let eur = slots_for_day(&pts, day, &chrono_tz::Europe::Amsterdam, 21.0);
        assert_eq!(eur.len(), SLOTS_PER_DAY);
        assert_eq!(eur.iter().filter(|v| v.is_finite()).count(), 96);
        // fixture: 2026-09-11 00:00 local is 183.9 €/MWh, 14:00 local is 154.42
        assert!((eur[0] - 0.1839 * 1.21).abs() < 1e-5, "{}", eur[0]);
        assert!((eur[56] - 0.15442 * 1.21).abs() < 1e-5, "{}", eur[56]);
        // the two other days are not in today's slots
        let other = NaiveDate::from_ymd_opt(2026, 9, 12).unwrap();
        assert!(slots_for_day(&pts, other, &chrono_tz::Europe::Amsterdam, 21.0)
            .iter()
            .all(|v| v.is_nan()));
        // the mean over the three requested days, VAT included
        let days = [
            NaiveDate::from_ymd_opt(2026, 9, 9).unwrap(),
            NaiveDate::from_ymd_opt(2026, 9, 10).unwrap(),
            day,
        ];
        let avg = mean_over_days(&pts, &days, &chrono_tz::Europe::Amsterdam, 21.0);
        assert!((avg - 0.217646).abs() < 1e-5, "{avg}");
        // only today: a different mean; no matching day: NaN
        let today_only = mean_over_days(&pts, &[day], &chrono_tz::Europe::Amsterdam, 0.0);
        assert!(today_only.is_finite() && (today_only - avg).abs() > 1e-4);
        assert!(mean_over_days(&pts, &[other], &chrono_tz::Europe::Amsterdam, 0.0).is_nan());
        assert!((eur_per_kwh(100.0, 0.0) - 0.1).abs() < 1e-7);
        assert!((eur_per_kwh(100.0, 21.0) - 0.121).abs() < 1e-7);
    }

    #[test]
    fn quarter_hour_slots_survive_dst_days() {
        let ams = chrono_tz::Europe::Amsterdam;
        // spring forward, 2026-03-29: 02:00 local does not exist
        let day = NaiveDate::from_ymd_opt(2026, 3, 29).unwrap();
        let pts = vec![
            (Utc.with_ymd_and_hms(2026, 3, 29, 0, 0, 0).unwrap(), 100.0), // 01:00 CET
            (Utc.with_ymd_and_hms(2026, 3, 29, 1, 0, 0).unwrap(), 200.0), // 03:00 CEST
        ];
        let eur = slots_for_day(&pts, day, &ams, 0.0);
        assert!((eur[4] - 0.1).abs() < 1e-6, "01:00 is slot 4");
        assert!(eur[8..12].iter().all(|v| v.is_nan()), "02:xx never happens");
        assert!((eur[12] - 0.2).abs() < 1e-6, "03:00 is slot 12");
        // fall back, 2026-10-25: 02:30 local happens twice and the two average
        let day = NaiveDate::from_ymd_opt(2026, 10, 25).unwrap();
        let pts = vec![
            (Utc.with_ymd_and_hms(2026, 10, 25, 0, 30, 0).unwrap(), 100.0), // 02:30 CEST
            (Utc.with_ymd_and_hms(2026, 10, 25, 1, 30, 0).unwrap(), 200.0), // 02:30 CET
        ];
        let eur = slots_for_day(&pts, day, &ams, 0.0);
        assert!((eur[10] - 0.15).abs() < 1e-6, "both 02:30s share slot 10: {}", eur[10]);
        assert_eq!(eur.iter().filter(|v| v.is_finite()).count(), 1);
    }

    #[test]
    fn the_local_zone_buckets_into_the_slot_the_gauge_reads() {
        use chrono::Timelike;
        // `runloop` reads the current slot from `chrono::Local`; bucketing has
        // to agree with it, whatever the Pi's system zone is
        let now = Local::now();
        let eur = slots_for_day(
            &[(now.with_timezone(&Utc), 100.0)],
            now.date_naive(),
            &Local,
            0.0,
        );
        let slot = (now.hour() * 4 + now.minute() / 15) as usize;
        assert!((eur[slot] - 0.1).abs() < 1e-6, "slot {slot} of {eur:?}");
        assert_eq!(eur.iter().filter(|v| v.is_finite()).count(), 1);
    }
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p rackscreen-sources prices::tests::energy_charts`
Expected: compile errors, `cannot find function parse_energy_charts`.

- [ ] **Step 3: Implement**

In `crates/sources/src/prices.rs`, add after `const ENTSOE_URL` (imports at the top of the file already bring `DateTime, Local, NaiveDate, NaiveDateTime, TimeZone, Timelike, Utc`, `Value`, `Context`, `Result`):

```rust
const ENERGY_CHARTS_URL: &str = "https://api.energy-charts.info/v2/price";
/// Quarter-hours in a local day; the day-ahead market's time unit.
pub const SLOTS_PER_DAY: usize = 96;

/// Energy-Charts `/v2/price` (schema 2.0) -> (UTC time, €/MWh), one per
/// quarter-hour. Entries without a timestamp or with a null price are skipped.
pub fn parse_energy_charts(json: &str) -> Result<Vec<(DateTime<Utc>, f32)>> {
    let v: Value = serde_json::from_str(json).context("energy-charts json")?;
    let data = v
        .get("data")
        .and_then(Value::as_array)
        .context("data missing")?;
    let out: Vec<(DateTime<Utc>, f32)> = data
        .iter()
        .filter_map(|p| {
            let ts = p.get("timestamp")?.as_str()?;
            let t = DateTime::parse_from_rfc3339(ts).ok()?.with_timezone(&Utc);
            let price = p.get("values")?.get("day_ahead_price")?.as_f64()? as f32;
            Some((t, price))
        })
        .collect();
    anyhow::ensure!(!out.is_empty(), "no price points in Energy-Charts document");
    Ok(out)
}

/// The exchange price per MWh as a consumer price per kWh with VAT on top.
pub fn eur_per_kwh(eur_per_mwh: f32, vat_pct: f32) -> f32 {
    eur_per_mwh / 1000.0 * (1.0 + vat_pct / 100.0)
}

/// Bucket €/MWh samples into the 96 local quarter-hours of `day` as €/kWh
/// with VAT. Missing slots are NaN; several samples in one slot (the repeated
/// hour of a fall-back day) average.
pub fn slots_for_day<Z: TimeZone>(
    samples: &[(DateTime<Utc>, f32)],
    day: NaiveDate,
    tz: &Z,
    vat_pct: f32,
) -> Vec<f32> {
    let mut sum = [0.0f32; SLOTS_PER_DAY];
    let mut cnt = [0u32; SLOTS_PER_DAY];
    for (t, mwh) in samples {
        let local = t.with_timezone(tz);
        if local.date_naive() != day {
            continue;
        }
        let slot = (local.hour() * 4 + local.minute() / 15) as usize;
        sum[slot] += eur_per_kwh(*mwh, vat_pct);
        cnt[slot] += 1;
    }
    (0..SLOTS_PER_DAY)
        .map(|i| {
            if cnt[i] == 0 {
                f32::NAN
            } else {
                sum[i] / cnt[i] as f32
            }
        })
        .collect()
}

/// Mean €/kWh (VAT included) over every sample whose local date is one of
/// `days`; NaN when none is.
pub fn mean_over_days<Z: TimeZone>(
    samples: &[(DateTime<Utc>, f32)],
    days: &[NaiveDate],
    tz: &Z,
    vat_pct: f32,
) -> f32 {
    let vals: Vec<f32> = samples
        .iter()
        .filter(|(t, _)| days.contains(&t.with_timezone(tz).date_naive()))
        .map(|(_, mwh)| eur_per_kwh(*mwh, vat_pct))
        .collect();
    if vals.is_empty() {
        f32::NAN
    } else {
        vals.iter().sum::<f32>() / vals.len() as f32
    }
}
```

`ENERGY_CHARTS_URL` is unused until Task 4; add `#[allow(dead_code)]` on it for now and remove the attribute in Task 4.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p rackscreen-sources prices::`
Expected: the four new tests pass; the old EnergyZero / ENTSO-E tests still pass.

- [ ] **Step 5: Commit**

```bash
git add crates/sources/src/prices.rs crates/sources/tests/fixtures/energy-charts-price.json
git commit -m "feat(sources): Energy-Charts price parser, quarter-hour slots and a multi-day mean

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 4: Switch the price data to euro quarter-hours, the needle model and the gauge scene

Everything that touches `Event::Prices` changes together so the workspace builds at the end of this task. Order the edits as listed; run the full workspace test only at the end.

**Files:**
- Modify: `crates/core/src/event.rs:181-186`
- Modify: `crates/core/src/model.rs` (`PriceState`, fields, `set_local_hour`, `tick`, `apply`, tests)
- Modify: `crates/core/src/scene_electricity.rs` (`price_scene` and its tests; `PRICE_SEGS`, `price_badge_text` removed)
- Modify: `crates/core/src/scene.rs:447`
- Modify: `crates/app/src/runloop.rs:88-96,178-179`
- Modify: `crates/app/examples/gifs.rs:43-44,177`
- Modify: `crates/render/tests/golden.rs:369-380`
- Modify: `crates/sources/src/fake.rs:52-57,361-369` and its tests
- Modify: `crates/sources/src/prices.rs` (drop EnergyZero / ENTSO-E, new `PriceConfig`, `run_prices`)
- Modify: `crates/sources/Cargo.toml` (drop `quick-xml`)
- Delete: `crates/sources/tests/fixtures/energyzero.json`, `entsoe.xml`, `entsoe-a03.xml`
- Modify: `crates/app/src/config.rs` (`PriceCfg`, defaults, validation, tests)
- Modify: `crates/app/src/run.rs:221-229,296-323`

**Interfaces:**
- Consumes: `price_level`, `PriceLevel` (Task 2); `parse_energy_charts`, `slots_for_day`, `mean_over_days`, `SLOTS_PER_DAY` (Task 3); `Drawable::Text` (Task 1).
- Produces: `Event::Prices { date: String, eur_per_kwh: Vec<f32>, avg_eur_per_kwh: f32, currency: String }`.
- Produces: `PriceState { date, eur: Vec<f32>, avg: f32, currency, have }`; `Model::set_local_slot(u32)`, `Model::local_slot() -> u32`, `Model::current_price() -> Option<f32>`, `Model::smooth_price_needle(now) -> f32`.
- Produces: `price_scene(model: &Model, now: Secs) -> Scene`, `GAUGE_SEGS`, `GAUGE_START_DEG`, `GAUGE_PITCH_DEG`.
- Produces: `PriceConfig { zone: String, vat_pct: f32, poll_secs: u64, tz: Local }`; `run_prices(cfg, ctx)` unchanged in signature.
- Produces: `PriceCfg { enabled: bool, zone: String, vat_pct: f32, poll_secs: u64 }` in `rackscreen-app`.

- [ ] **Step 1: Event**

Replace the `Prices` variant in `crates/core/src/event.rs`:

```rust
    /// Day-ahead prices, one entry per local quarter-hour of `date` starting
    /// at 00:00 (NaN where unknown), plus the mean over that day and the two
    /// before it.
    Prices {
        date: String,
        eur_per_kwh: Vec<f32>,
        avg_eur_per_kwh: f32,
        currency: String,
    },
```

- [ ] **Step 2: Model state, slot and needle**

In `crates/core/src/model.rs`:

Replace `PriceState`:

```rust
/// Day-ahead prices, one entry per local quarter-hour starting at 00:00.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PriceState {
    pub date: String,
    pub eur: Vec<f32>,
    /// Mean €/kWh over today and the two days before; the level's reference.
    pub avg: f32,
    pub currency: String,
    pub have: bool,
}
```

Change the import `use crate::electricity::{partition, Section, Source};` to
`use crate::electricity::{partition, price_level, Section, Source};`.

In `struct Model`, replace the field `local_hour: u32,` with:

```rust
    /// The local quarter-hour (0..=95) the price gauge reads as "now".
    local_slot: u32,
    /// Needle position on the price gauge, eased between slots.
    price_needle: Smooth,
```

In `Model::new`, replace `local_hour: 12,` with:

```rust
            local_slot: 48,
            price_needle: Smooth::new(0.5, SMOOTH_SECS),
```

Replace `set_local_hour` / `local_hour`:

```rust
    /// The local quarter-hour (0..=95) the price gauge reads as "now".
    pub fn set_local_slot(&mut self, s: u32) {
        self.local_slot = s.min(95);
    }
    pub fn local_slot(&self) -> u32 {
        self.local_slot
    }
    /// The price of the current quarter-hour, when it is known.
    pub fn current_price(&self) -> Option<f32> {
        if !self.prices.have {
            return None;
        }
        self.prices
            .eur
            .get(self.local_slot as usize)
            .copied()
            .filter(|v| v.is_finite())
    }
    /// The eased needle position of the price gauge, `0..=1`.
    pub fn smooth_price_needle(&self, now: Secs) -> f32 {
        self.price_needle.value(now)
    }
    /// Point the needle at the current slot's level; a slot with no price
    /// leaves it where it is.
    fn retarget_price_needle(&mut self, now: Secs) {
        if let Some(p) = self.current_price() {
            let (_, t) = price_level(p, self.prices.avg);
            self.price_needle.set(t, now);
        }
    }
```

In `tick`, add as the first statement:

```rust
        self.retarget_price_needle(now);
```

Replace the `Event::Prices` arm in `apply`:

```rust
            Event::Prices {
                date,
                eur_per_kwh,
                avg_eur_per_kwh,
                currency,
            } => {
                self.prices = PriceState {
                    date,
                    eur: eur_per_kwh,
                    avg: avg_eur_per_kwh,
                    currency,
                    have: true,
                };
                self.retarget_price_needle(now);
            }
```

Update the test `prices_and_links_and_token`: replace its body from the first `assert_eq!(m.local_hour() ...` through the `set_local_hour(99)` assertions with:

```rust
        assert_eq!(m.local_slot(), 48, "noon until the render loop says otherwise");
        assert_eq!(m.current_price(), None);
        let mut eur = vec![f32::NAN; 96];
        eur[48] = 0.221;
        eur[49] = 0.100;
        m.apply(
            Event::Prices {
                date: "2026-09-07".into(),
                eur_per_kwh: eur,
                avg_eur_per_kwh: 0.20,
                currency: "EUR".into(),
            },
            0.0,
        );
        assert!(m.prices().have);
        assert_eq!(m.prices().eur.len(), 96);
        assert_eq!(m.prices().avg, 0.20);
        assert_eq!(m.prices().currency, "EUR");
        assert_eq!(m.prices().date, "2026-09-07");
        assert_eq!(m.current_price(), Some(0.221));
        // the needle eased straight to the slot's level (ratio 1.105: PRICEY, t = 0.6 + 0.045/0.25/5)
        assert!((m.smooth_price_needle(5.0) - 0.636).abs() < 1e-3);
        m.apply(
            Event::Link {
                target: LinkTarget::Electricity,
                up: true,
            },
            0.0,
        );
        m.apply(
            Event::Link {
                target: LinkTarget::Prices,
                up: true,
            },
            0.0,
        );
        assert!(m.link().electricity && m.link().prices);
        // the next slot retargets the needle on tick, and it eases rather than jumps
        m.set_local_slot(49);
        assert_eq!(m.current_price(), Some(0.100));
        m.tick(5.0);
        let mid = m.smooth_price_needle(5.2);
        assert!(mid < 0.636 && mid > 0.0, "easing down: {mid}");
        // ratio 0.5: V.CHEAP, 0.2 of the 0.3-wide band, t = 0.667 / 5
        assert!((m.smooth_price_needle(6.0) - 0.1333).abs() < 1e-3);
        m.set_local_slot(99);
        assert_eq!(m.local_slot(), 95, "clamped into the day");
```

Keep the existing `Link` assertions that follow if they are still present in the test; remove any remaining `local_hour` references.

- [ ] **Step 3: The gauge scene and its tests**

In `crates/core/src/scene_electricity.rs`:

Replace the imports at the top:

```rust
use crate::anim::{breathe, Secs};
use crate::electricity::{partition, price_level, PriceLevel, MIX_ICON_MIN_SEGS};
use crate::model::Model;
use crate::scene::{badge, badge_w, icon_at, ring, ring_states, seg_count, Drawable, Scene, SegState};
use crate::theme::layout::*;
use crate::theme::{carbon_color, Color, BLUE, GREEN, GREY, OFF, WHITE};
```

(`price_color` is no longer imported; if it is now unused in `theme.rs` leave it, it has its own test.)

Replace everything from `pub const PRICE_SEGS: usize = 48;` through the end of the old `price_scene` (line 142) with:

```rust
/// The level gauge: 45 segments at 6°, five bands of nine, from 7:30 to 4:30 o'clock.
pub const GAUGE_SEGS: usize = 45;
pub const GAUGE_START_DEG: f32 = 225.0;
pub const GAUGE_PITCH_DEG: f32 = 6.0;
const SEGS_PER_BAND: usize = GAUGE_SEGS / 5;
/// Angle the needle sweeps from `t = 0` (segment 0) to `t = 1` (the last segment).
const GAUGE_SWEEP_DEG: f32 = (GAUGE_SEGS - 1) as f32 * GAUGE_PITCH_DEG;
const NEEDLE_R0: f32 = 88.0;
const NEEDLE_R1: f32 = 116.0;
const NEEDLE_W: f32 = 4.0;
const DIM_BAND_ALPHA: f32 = 0.45;
const PRICE_TEXT_CY: f32 = 100.0;
const PRICE_TEXT_PX: f32 = 40.0;
const PRICE_CAPTION_CY: f32 = 130.0;
const PRICE_CAPTION_PX: f32 = 11.0;
const LEVEL_BADGE_W: f32 = 84.0;
const LEVEL_BADGE_PX: f32 = 13.0;

/// Angle of the needle for position `t`.
pub fn needle_angle(t: f32) -> f32 {
    GAUGE_START_DEG + GAUGE_SWEEP_DEG * t.clamp(0.0, 1.0)
}

pub fn price_scene(model: &Model, now: Secs) -> Scene {
    let p = model.prices();
    let cur = model.current_price();
    let level = cur.map(|v| price_level(v, p.avg).0);
    let t = model.smooth_price_needle(now);
    let needle_seg = ((t * (GAUGE_SEGS - 1) as f32).round() as usize).min(GAUGE_SEGS - 1);
    let mut states = Vec::with_capacity(GAUGE_SEGS);
    for i in 0..GAUGE_SEGS {
        let band = i / SEGS_PER_BAND;
        let gap = band < 4 && i % SEGS_PER_BAND == SEGS_PER_BAND - 1;
        if gap {
            states.push(SegState::Off);
            continue;
        }
        let color = PriceLevel::ALL[band].color();
        let alpha = match level {
            Some(lvl) if lvl.index() == band && i == needle_seg => breathe(now, 2.4),
            Some(lvl) if lvl.index() == band => 1.0,
            _ => DIM_BAND_ALPHA,
        };
        states.push(SegState::On(color, alpha));
    }
    let mut s = Scene::new();
    s.push(Drawable::Ring {
        cx: CX,
        cy: CY,
        radius: RING_R,
        n: GAUGE_SEGS,
        states,
        pitch_deg: GAUGE_PITCH_DEG,
        start_deg: GAUGE_START_DEG,
    });
    if level.is_some() {
        s.push(Drawable::Tick {
            cx: CX,
            cy: CY,
            angle_deg: needle_angle(t),
            r0: NEEDLE_R0,
            r1: NEEDLE_R1,
            width: NEEDLE_W,
            color: WHITE,
            alpha: 1.0,
        });
    }
    let (text, color) = match cur {
        Some(v) if v < 0.0 => (format!("{v:.3}"), BLUE),
        Some(v) => (format!("{v:.3}"), WHITE),
        None => ("--".to_string(), GREY),
    };
    s.push(Drawable::Text {
        cx: CX,
        cy: PRICE_TEXT_CY,
        text,
        px: PRICE_TEXT_PX,
        color,
        alpha: 1.0,
    });
    s.push(Drawable::Text {
        cx: CX,
        cy: PRICE_CAPTION_CY,
        text: "€/kWh".to_string(),
        px: PRICE_CAPTION_PX,
        color: GREY,
        alpha: 1.0,
    });
    let (word, stroke) = match level {
        Some(lvl) => (lvl.word().to_string(), lvl.color()),
        None => ("no data".to_string(), GREY),
    };
    let mut b = badge_w(BADGE_CY, stroke, word, LEVEL_BADGE_W);
    if let Drawable::Badge { text_px, .. } = &mut b {
        *text_px = LEVEL_BADGE_PX;
    }
    s.push(b);
    s
}
```

In `mod tests`, replace `price_model` and the four old price tests (`price_ring_has_two_segments_per_hour_and_dims_the_past`, `price_badge_alternates_with_the_daily_minimum`, `missing_hours_are_dark_and_the_badge_says_nothing`, `negative_prices_are_blue`) plus the `day()` helper with:

```rust
    /// A model at slot 56 (14:00) with a flat day at `flat` €/kWh except slot
    /// 56, and a three-day average of 0.20.
    fn price_model(at_now: f32) -> Model {
        let mut m = Model::new(Thresholds::default());
        m.set_local_slot(56);
        let mut eur = vec![0.15f32; 96];
        eur[56] = at_now;
        m.apply(
            Event::Prices {
                date: "2026-09-07".into(),
                eur_per_kwh: eur,
                avg_eur_per_kwh: 0.20,
                currency: "EUR".into(),
            },
            0.0,
        );
        m
    }

    fn gauge_states(s: &Scene) -> Vec<SegState> {
        s.items
            .iter()
            .find_map(|d| match d {
                Drawable::Ring { states, .. } => Some(states.clone()),
                _ => None,
            })
            .expect("ring")
    }

    fn needle(s: &Scene) -> Option<f32> {
        s.items.iter().find_map(|d| match d {
            Drawable::Tick { angle_deg, .. } => Some(*angle_deg),
            _ => None,
        })
    }

    fn texts(s: &Scene) -> Vec<(String, Color)> {
        s.items
            .iter()
            .filter_map(|d| match d {
                Drawable::Text { text, color, .. } => Some((text.clone(), *color)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn price_gauge_has_five_bands_with_gaps_and_lights_the_needle_band() {
        // 0.221 / 0.20 = 1.105: PRICEY, band 3
        let m = price_model(0.221);
        let s = price_scene(&m, 5.0);
        let states = gauge_states(&s);
        assert_eq!(states.len(), GAUGE_SEGS);
        for band in 0..4 {
            assert_eq!(states[band * 9 + 8], SegState::Off, "gap after band {band}");
        }
        assert!(matches!(states[44], SegState::On(..)), "the last band has no gap");
        assert!(matches!(states[0], SegState::On(c, a) if c == GREEN && (a - 0.45).abs() < 1e-6));
        assert!(matches!(states[18], SegState::On(c, a) if c == crate::theme::AMBER && (a - 0.45).abs() < 1e-6));
        assert!(matches!(states[27], SegState::On(c, a) if c == PriceLevel::Pricey.color() && a == 1.0));
        assert!(matches!(states[44], SegState::On(c, a) if c == crate::theme::RED && (a - 0.45).abs() < 1e-6));
        // t = 0.6 + (0.105 / 0.25) / 5 = 0.684; needle segment round(0.684 * 44) = 30 breathes
        assert!(matches!(states[30], SegState::On(_, a) if a < 1.0), "needle segment breathes");
        let angle = needle(&s).expect("needle");
        assert!((angle - (225.0 + 264.0 * 0.684)).abs() < 0.5, "{angle}");
        assert_eq!(badge_text(&s), "PRICEY");
        assert!(s
            .items
            .iter()
            .any(|d| matches!(d, Drawable::Badge { stroke, w, .. } if *stroke == PriceLevel::Pricey.color() && *w == 84.0)));
        let t = texts(&s);
        assert_eq!(t[0], ("0.221".to_string(), WHITE));
        assert_eq!(t[1].0, "€/kWh");
        assert!(icons(&s).is_empty(), "no euro icon any more");
    }

    #[test]
    fn price_gauge_words_follow_the_bands() {
        // needle segments for these prices are 6, 13, 21, 30 and 38: never the
        // first segment of a band, so that one is lit at exactly 1.0
        for (price, word, band) in [
            (0.10, "V.CHEAP", 0usize),
            (0.15, "CHEAP", 1),
            (0.20, "NORMAL", 2),
            (0.25, "PRICEY", 3),
            (0.30, "V.PRICEY", 4),
        ] {
            let s = price_scene(&price_model(price), 5.0);
            assert_eq!(badge_text(&s), word);
            let states = gauge_states(&s);
            assert!(
                matches!(states[band * 9], SegState::On(c, a) if c == PriceLevel::ALL[band].color() && a == 1.0),
                "band {band} lit for {price}: {:?}",
                states[band * 9]
            );
            let other = if band == 0 { 9 } else { 0 };
            assert!(
                matches!(states[other], SegState::On(_, a) if (a - 0.45).abs() < 1e-6),
                "other bands dim for {price}"
            );
        }
    }

    #[test]
    fn negative_price_is_blue_and_very_cheap() {
        let s = price_scene(&price_model(-0.006), 5.0);
        assert_eq!(texts(&s)[0], ("-0.006".to_string(), BLUE));
        assert_eq!(badge_text(&s), "V.CHEAP");
        assert!((needle(&s).unwrap() - 225.0).abs() < 0.5, "pinned at the cheap end");
    }

    #[test]
    fn price_gauge_without_data_hides_the_needle() {
        let mut m = Model::new(Thresholds::default());
        m.set_local_slot(56);
        let s = price_scene(&m, 5.0);
        assert!(needle(&s).is_none());
        assert_eq!(texts(&s)[0], ("--".to_string(), GREY));
        assert_eq!(badge_text(&s), "no data");
        assert!(gauge_states(&s)
            .iter()
            .all(|st| matches!(st, SegState::Off | SegState::On(_, a) if (*a - 0.45).abs() < 1e-6)));
        // a day with a hole at the current slot reads the same way
        let mut eur = vec![0.15f32; 96];
        eur[56] = f32::NAN;
        m.apply(
            Event::Prices {
                date: "2026-09-07".into(),
                eur_per_kwh: eur,
                avg_eur_per_kwh: 0.20,
                currency: "EUR".into(),
            },
            0.0,
        );
        let s = price_scene(&m, 5.0);
        assert!(needle(&s).is_none());
        assert_eq!(badge_text(&s), "no data");
    }
```

The `badge_text` and `icons` helpers already exist in that module; keep them. Add `use crate::theme::Color;` to the tests module if `Color` is not already in scope there.

In `crates/core/src/scene.rs` line 447 change the dispatch to:

```rust
        Role::Price => crate::scene_electricity::price_scene(model, now),
```

- [ ] **Step 4: Runloop, gifs, golden**

`crates/app/src/runloop.rs`: change `clock()` to drop the hour,

```rust
fn clock() -> (u32, i64, i32) {
    let t = chrono::Local::now();
    (
        t.hour() * 60 + t.minute(),
        t.timestamp(),
        t.offset().local_minus_utc(),
    )
}
```

and the call site:

```rust
            let (minutes, unix, offset) = clock();
            model.set_local_slot(minutes / 15);
```

If `minutes` is used elsewhere in the loop (night window), keep it as is: only the tuple shape changes. If `Timelike` was imported solely for `hour()`, it is still needed for `minute()`.

`crates/app/examples/gifs.rs`: replace the `HOUR` constant and its use:

```rust
/// Local quarter-hour the price gauge reads as "now": 14:00.
const SLOT: u32 = 56;
```

```rust
    model.set_local_slot(SLOT);
```

`crates/render/tests/golden.rs`, in `electricity_model()`, replace the price block and the `set_local_hour(14)` line with:

```rust
    // 14:00 on a day that climbs from 0.062 to 0.292 €/kWh, 0.221 at 14:00,
    // against a three-day mean of 0.18: PRICEY
    m.set_local_slot(56);
    let mut eur: Vec<f32> = (0..96).map(|i| (6.2 + (i / 4) as f32) / 100.0).collect();
    eur[56] = 0.221;
    m.apply(
        Event::Prices {
            date: "2026-09-07".into(),
            eur_per_kwh: eur,
            avg_eur_per_kwh: 0.18,
            currency: "EUR".into(),
        },
        0.0,
    );
```

`set_local_slot` goes before `apply` so the needle is targeted on apply. In `price_carbon_and_renewable`, change the comment above the price render to `// 1.2 s: the needle has eased onto its band.` and keep `scene_for_role(Role::Price, 1.2)`.

- [ ] **Step 5: Fake source**

`crates/sources/src/fake.rs`: replace `PRICE_CURVE`:

```rust
/// Day-ahead price curve in €/kWh per hour; `prices()` interpolates it to quarter-hours.
const PRICE_CURVE: [f32; 24] = [
    0.22, 0.19, 0.17, 0.16, 0.15, 0.15, 0.17, 0.21, 0.24, 0.22, 0.18, 0.13, 0.09, 0.07, 0.06,
    0.08, 0.12, 0.19, 0.26, 0.29, 0.27, 0.24, 0.22, 0.21,
];
```

and `prices()`:

```rust
    fn prices(&self) -> Event {
        let mut hourly = PRICE_CURVE.to_vec();
        hourly.rotate_left(self.price_rot % PRICE_CURVE.len());
        let eur = (0..96)
            .map(|i| {
                let h = i / 4;
                let a = hourly[h];
                let b = hourly[(h + 1) % 24];
                a + (b - a) * (i % 4) as f32 / 4.0
            })
            .collect();
        Event::Prices {
            date: PRICE_DATE.into(),
            eur_per_kwh: eur,
            avg_eur_per_kwh: PRICE_CURVE.iter().sum::<f32>() / PRICE_CURVE.len() as f32,
            currency: "EUR".into(),
        }
    }
```

Tests in the same file: the `matches!` on `Event::Prices` around line 1013 becomes

```rust
            Event::Prices { eur_per_kwh, avg_eur_per_kwh, currency, .. }
                if eur_per_kwh.len() == 96 && eur_per_kwh[0] == 0.22 && *avg_eur_per_kwh > 0.17 && *avg_eur_per_kwh < 0.19 && currency == "EUR"
```

and in `tick_emits_temps_storage_electricity_and_prices` the extraction becomes `Event::Prices { eur_per_kwh, .. } => Some(eur_per_kwh.clone())` with `assert_eq!(rotated[0], 0.19, "curve rotated by one hour");` and add `assert!((rotated[2] - 0.18).abs() < 1e-6, "quarter-hours interpolate toward the next hour");`.

- [ ] **Step 6: The source loop**

`crates/sources/src/prices.rs`: delete `PriceSource`, `ENTSOE_URL`, `entsoe_zone_for`, `parse_utc`, `parse_energyzero`, `parse_entsoe`, `fill_period`, `hourly_ct_for_day`, `day_bounds_utc`, `redact`, `fetch_day` and their tests (`energyzero_to_local_hours`, `entsoe_points_and_units`, `request_errors_never_carry_the_token`, `entsoe_a03_carries_the_missing_positions_forward`, `the_local_zone_buckets_into_the_hour_the_marker_shows`, `missing_hours_are_nan_and_zones_map`) and the `EZ`, `ENTSOE`, `ENTSOE_A03` constants. Remove `#[allow(dead_code)]` from `ENERGY_CHARTS_URL`. Delete the three old fixture files. Remove `quick-xml = "0.42"` from `crates/sources/Cargo.toml` (verify first: `grep -rn quick_xml crates --include=*.rs` must only hit `prices.rs`).

Module doc and the config become:

```rust
//! Day-ahead electricity prices from Energy-Charts (Fraunhofer ISE), no token.

use std::time::Duration;

use anyhow::{Context, Result};
use chrono::{DateTime, Local, NaiveDate, TimeZone, Timelike, Utc};
use rackscreen_core::event::{Event, LinkTarget};
use serde_json::Value;

use crate::http::client;
use crate::SourceCtx;

#[derive(Clone, Debug)]
pub struct PriceConfig {
    /// Energy-Charts bidding zone, for example `NL` or `DE-LU`.
    pub zone: String,
    /// VAT added to the raw exchange price.
    pub vat_pct: f32,
    pub poll_secs: u64,
    /// The system local zone, the same clock `runloop` uses for the current
    /// slot; set the Pi's zone with `timedatectl`.
    pub tz: Local,
}
```

The fetch and loop:

```rust
/// One request for the local days `first..=last`, inclusive.
async fn fetch_days(
    cfg: &PriceConfig,
    first: NaiveDate,
    last: NaiveDate,
) -> Result<Vec<(DateTime<Utc>, f32)>> {
    let body = client()
        .get(ENERGY_CHARTS_URL)
        .query(&[
            ("bzn", cfg.zone.as_str()),
            ("start", &first.to_string()),
            ("end", &last.to_string()),
        ])
        .send()
        .await
        .context("energy-charts request")?
        .error_for_status()
        .context("energy-charts status")?
        .text()
        .await
        .context("energy-charts body")?;
    parse_energy_charts(&body)
}

pub async fn run_prices(cfg: PriceConfig, ctx: SourceCtx) {
    let mut failures = 0u32;
    loop {
        if ctx.shutdown.is_cancelled() {
            return;
        }
        let today = Utc::now().with_timezone(&cfg.tz).date_naive();
        let first = today - chrono::Duration::days(2);
        let days = [first, first + chrono::Duration::days(1), today];
        match fetch_days(&cfg, first, today).await {
            Ok(samples) => {
                failures = 0;
                let eur = slots_for_day(&samples, today, &cfg.tz, cfg.vat_pct);
                let avg = mean_over_days(&samples, &days, &cfg.tz, cfg.vat_pct);
                tracing::info!(
                    "prices: {} of {SLOTS_PER_DAY} quarter-hours for {today}, three-day mean {avg:.3} €/kWh",
                    eur.iter().filter(|v| v.is_finite()).count()
                );
                ctx.emit(Event::Link {
                    target: LinkTarget::Prices,
                    up: true,
                });
                ctx.emit(Event::Prices {
                    date: today.to_string(),
                    eur_per_kwh: eur,
                    avg_eur_per_kwh: avg,
                    currency: "EUR".into(),
                });
            }
            Err(e) => {
                failures += 1;
                tracing::warn!("prices: {e:#}");
                if failures >= 2 {
                    ctx.emit(Event::Link {
                        target: LinkTarget::Prices,
                        up: false,
                    });
                }
            }
        }
        // the gauge follows the wall clock on its own; re-poll on the interval,
        // and at the latest just after local midnight for the new day
        let now_local = Utc::now().with_timezone(&cfg.tz);
        let to_midnight = 86_400
            - (now_local.hour() * 3600 + now_local.minute() * 60 + now_local.second()) as u64
            + 5;
        let normal = cfg.poll_secs.max(900);
        let wait = if failures > 0 {
            120
        } else {
            normal.min(to_midnight)
        };
        tokio::select! {
            _ = ctx.shutdown.cancelled() => return,
            _ = tokio::time::sleep(Duration::from_secs(wait)) => {}
        }
    }
}
```

Keep the existing `Err` branch body exactly as it was if it has more (the `Link` down emit after two failures is what it does today).

- [ ] **Step 7: Config**

`crates/app/src/config.rs`: replace `PriceCfg`:

```rust
/// Day-ahead electricity price from Energy-Charts.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct PriceCfg {
    pub enabled: bool,
    /// Energy-Charts bidding zone (`NL`, `DE-LU`, `DK1`, ...); empty derives
    /// it from `electricity.zone`.
    pub zone: String,
    /// VAT added to the raw exchange price, percent.
    pub vat_pct: f32,
    pub poll_secs: u64,
}
```

its default:

```rust
impl Default for PriceCfg {
    fn default() -> Self {
        Self {
            enabled: true,
            zone: String::new(),
            vat_pct: 21.0,
            poll_secs: 900,
        }
    }
}
```

and in `validate`, replace the `price.source` ensure with:

```rust
        anyhow::ensure!(
            (0.0..=100.0).contains(&self.price.vat_pct),
            "price.vat_pct must be between 0 and 100 (got {})",
            self.price.vat_pct
        );
        anyhow::ensure!(
            self.price.poll_secs >= 900,
            "price.poll_secs must be at least 900; Energy-Charts rate-limits (got {})",
            self.price.poll_secs
        );
```

Tests: in `electricity_price_and_threshold_defaults` replace the two price asserts with

```rust
        assert!(c.price.enabled);
        assert_eq!(c.price.zone, "");
        assert_eq!(c.price.vat_pct, 21.0);
        assert_eq!(c.price.poll_secs, 900);
```

and add:

```rust
    #[test]
    fn price_bounds_and_old_keys() {
        let mut c = Config::default();
        c.price.vat_pct = 120.0;
        assert!(c.validate().unwrap_err().to_string().contains("price.vat_pct"));
        c.price.vat_pct = 0.0;
        c.price.poll_secs = 600;
        assert!(c.validate().unwrap_err().to_string().contains("price.poll_secs"));
        c.price.poll_secs = 900;
        c.validate().unwrap();
        // a file from before Energy-Charts still loads; its old keys are ignored
        let old = Config::from_yaml(
            "price:\n  source: entsoe\n  entsoe_token: t\n  entsoe_zone: 10YNL----------L\n  include_vat: false\n  poll_secs: 1800\n",
        )
        .unwrap();
        assert_eq!(old.price.poll_secs, 1800);
        assert!(old.price.enabled);
        assert_eq!(old.price.vat_pct, 21.0);
        assert!(!old.to_yaml().unwrap().contains("entsoe"), "dropped on save");
    }
```

- [ ] **Step 8: Wiring in `run.rs`**

Replace the `if let Some(source) = price_source(cfg) { ... }` block in `spawn_energy_sources`:

```rust
    if let Some(zone) = price_zone(cfg) {
        let pcfg = rackscreen_sources::prices::PriceConfig {
            zone,
            vat_pct: cfg.price.vat_pct,
            poll_secs: cfg.price.poll_secs,
            // the system local zone, the same clock the current slot uses
            tz: chrono::Local,
        };
        runtime.spawn(rackscreen_sources::prices::run_prices(pcfg, ctx.clone()));
    }
```

and replace `price_source` with:

```rust
/// The Energy-Charts bidding zone to poll: `price.zone` when set, else the one
/// `electricity.zone` maps to. `None` when prices are off, or (with a warning)
/// when neither gives a zone.
fn price_zone(cfg: &Config) -> Option<String> {
    if !cfg.price.enabled {
        return None;
    }
    let zone = cfg.price.zone.trim();
    if !zone.is_empty() {
        return Some(zone.to_ascii_uppercase());
    }
    match rackscreen_core::electricity::energy_charts_zone_for(&cfg.electricity.zone) {
        Some(z) => Some(z.to_string()),
        None => {
            tracing::warn!(
                "price zone unknown for electricity zone {}; set price.zone",
                cfg.electricity.zone
            );
            None
        }
    }
}
```

- [ ] **Step 9: Build, test, regenerate the golden**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean. Fix any leftover `ct_per_kwh` / `local_hour` / `PRICE_SEGS` references it reports (the setup crate does not compile yet if it references `price.source`: that is Task 5; if clippy fails only in `rackscreen-setup` on `price.source` / `entsoe_*`, do Task 5 before this step's commit and commit both together).

Run: `cargo test --workspace`
Expected: everything passes except `price_carbon_and_renewable`, which fails with `price: N of 57600 pixels differ (run with UPDATE_GOLDENS=1 to accept)`.

Run: `UPDATE_GOLDENS=1 cargo test -p rackscreen-render --test golden price_carbon_and_renewable`
Expected: `wrote golden .../price.png` (carbon and renewable are rewritten identically).

Look at the regenerated `crates/render/tests/golden/price.png`: five coloured bands with gaps, a white needle in the amber-red band, `0.221` large, `€/kWh` under it, badge `PRICEY`. Then `cargo test --workspace` again: green.

- [ ] **Step 10: Commit**

```bash
git add -A crates/core crates/render crates/sources crates/app
git commit -m "feat(price): level gauge on Energy-Charts quarter-hour prices

The price role becomes a five-band gauge with a needle at the current
slot's level against a three-day mean, the price in €/kWh and a
one-word badge. EnergyZero and ENTSO-E go; Energy-Charts needs no token
and covers the EU.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 5: Setup TUI fields and role hint

**Files:**
- Modify: `crates/setup/src/screens/configure.rs` (`FieldKind::Choice`, `PRICE_SOURCES`, `next_choice`, `Field` variants, `FIELDS` specs, `get`, `set`, key handling at ~831, rendering at ~997, tests)
- Modify: `crates/setup/src/screens/screens.rs:271-275` and the test `hints_flag_a_missing_token_and_a_dead_price_source`

**Interfaces:**
- Consumes: `PriceCfg { enabled, zone, vat_pct, poll_secs }` (Task 4), `rackscreen_core::electricity::energy_charts_zone_for` (Task 2).
- Produces: `Field::PriceEnabled`, `Field::PriceZone`, `Field::PriceVat` (with `Field::PricePoll` kept).

- [ ] **Step 1: Write the failing tests**

In `crates/setup/src/screens/configure.rs` tests, replace `electricity_price_and_temp_fields` and delete `choice_field_cycles_on_enter`:

```rust
    #[test]
    fn electricity_price_and_temp_fields() {
        let mut c = Config::default();
        assert_eq!(get(&c, Field::ElecZone), "NL");
        assert_eq!(get(&c, Field::PriceEnabled), "true");
        assert_eq!(get(&c, Field::PriceZone), "");
        assert_eq!(get(&c, Field::PriceVat), "21");
        assert_eq!(get(&c, Field::HotTemp), "70");
        set(&mut c, Field::ElecEnabled, "true").unwrap();
        assert!(c.electricity.enabled);
        set(&mut c, Field::ElecZone, " DE ").unwrap();
        assert_eq!(c.electricity.zone, "DE");
        set(&mut c, Field::ElecToken, "tok").unwrap();
        assert_eq!(c.electricity.token, "tok");
        set(&mut c, Field::ElecPoll, "10").unwrap();
        assert_eq!(c.electricity.poll_secs, 60, "poll secs are floored at 60");
        set(&mut c, Field::PriceEnabled, "false").unwrap();
        assert!(!c.price.enabled);
        set(&mut c, Field::PriceZone, " dk1 ").unwrap();
        assert_eq!(c.price.zone, "DK1", "trimmed and upper-cased");
        set(&mut c, Field::PriceVat, "0").unwrap();
        assert_eq!(c.price.vat_pct, 0.0);
        assert!(set(&mut c, Field::PriceVat, "150").unwrap_err().contains("0 and 100"));
        assert!(set(&mut c, Field::PriceVat, "lots").is_err());
        set(&mut c, Field::PricePoll, "60").unwrap();
        assert_eq!(c.price.poll_secs, 900, "floored at 900 for Energy-Charts");
        set(&mut c, Field::PricePoll, "1800").unwrap();
        assert_eq!(c.price.poll_secs, 1800);
        set(&mut c, Field::HotTemp, "82.5").unwrap();
        assert_eq!(c.thresholds.hot_temp, 82.5);
        assert!(set(&mut c, Field::HotTemp, "warm").is_err());
        assert!(c.validate().is_ok());
    }
```

In `crates/setup/src/screens/screens.rs`, replace `hints_flag_a_missing_token_and_a_dead_price_source`:

```rust
    #[test]
    fn hints_flag_a_missing_token_and_a_dead_price_zone() {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;
        let dir = tempfile::tempdir().unwrap();
        let sh = test_shared(dir.path());
        let mut s = Screens::new(&sh);
        // defaults: no Electricity Maps token, prices derived from zone NL
        assert_eq!(s.role_hint(Role::PowerMix), Some("no token"));
        assert_eq!(s.role_hint(Role::Carbon), Some("no token"));
        assert_eq!(s.role_hint(Role::Price), None, "NL maps to an Energy-Charts zone");
        assert_eq!(s.role_hint(Role::Cpu), None);
        {
            let cfg = s.cfg.as_mut().unwrap();
            cfg.electricity.token = "tok".into();
            cfg.electricity.zone = "XX".into();
        }
        assert_eq!(s.role_hint(Role::PowerMix), None);
        assert_eq!(
            s.role_hint(Role::Price),
            Some("no price zone"),
            "an unmapped electricity zone and no price zone cannot fetch"
        );
        s.cfg.as_mut().unwrap().price.zone = "DE-LU".into();
        assert_eq!(s.role_hint(Role::Price), None);
        s.cfg.as_mut().unwrap().price.enabled = false;
        assert_eq!(s.role_hint(Role::Price), Some("prices off"));
        // and the row says so
        s.editor.apply_preset(Preset::Electricity);
        let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
        term.draw(|f| s.draw(f, f.area(), &sh, 0.0)).unwrap();
        let text = term.backend().to_string();
        assert!(text.contains("! prices off"), "{text}");
    }
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p rackscreen-setup price`
Expected: compile errors on `Field::PriceEnabled`, `price.source`.

- [ ] **Step 3: Implement the fields**

In `configure.rs`:

- Delete the `Choice` variant of `FieldKind`, the `PRICE_SOURCES` const, `next_choice`, the `else if kind == FieldKind::Choice { ... }` branch in the Enter handler, and the `(_, _, FieldKind::Choice) => format!("‹ {raw} ›"),` render arm. Update the `use FieldKind::{Bool, Choice, Number, Secret, Text};` line to drop `Choice`.
- In `enum Field`, replace `PriceSource, EntsoeToken, EntsoeZone, IncludeVat,` with `PriceEnabled, PriceZone, PriceVat,`.
- In `FIELDS`, replace the four price specs with:

```rust
    spec(
        Field::PriceEnabled,
        Energy,
        "Prices",
        "enabled",
        Bool,
        "Day-ahead prices from Energy-Charts, no key.",
    ),
    spec(
        Field::PriceZone,
        Energy,
        "Prices",
        "zone",
        Text,
        "Energy-Charts bidding zone (NL, DE-LU, DK1); empty derives it from the electricity zone.",
    ),
    spec(
        Field::PriceVat,
        Energy,
        "Prices",
        "vat %",
        Number,
        "VAT added to the exchange price, 0 to 100.",
    ),
```

- In `get`, replace the four price arms with:

```rust
        Field::PriceEnabled => cfg.price.enabled.to_string(),
        Field::PriceZone => cfg.price.zone.clone(),
        Field::PriceVat => format!("{}", cfg.price.vat_pct),
```

- In `set`, replace the four price arms with:

```rust
        Field::PriceEnabled => cfg.price.enabled = t == "true",
        Field::PriceZone => cfg.price.zone = t.to_ascii_uppercase(),
        Field::PriceVat => {
            let v: f32 = num(t, "vat %")?;
            if !(0.0..=100.0).contains(&v) {
                return Err("vat % must be between 0 and 100".into());
            }
            cfg.price.vat_pct = v;
        }
```

and change the `PricePoll` arm's floor from `.max(60)` to `.max(900)`, with its spec hint (if it mentions 60) saying "Seconds between polls, at least 900."

If the spec `hint` for `PricePoll` reads "Seconds between polls, at least 60." change it accordingly.

- [ ] **Step 4: Role hint**

In `screens.rs`, replace the `Role::Price` arm:

```rust
            Role::Price => {
                if !c.price.enabled {
                    Some("prices off")
                } else if c.price.zone.trim().is_empty()
                    && rackscreen_core::electricity::energy_charts_zone_for(&c.electricity.zone)
                        .is_none()
                {
                    Some("no price zone")
                } else {
                    None
                }
            }
```

Update the doc comment above `role_hint` to: `/// The `!` hint for one role: the grid roles need an Electricity Maps token, and `price` needs to be on with a bidding zone it can resolve.`

- [ ] **Step 5: Run the setup tests and the workspace**

Run: `cargo test -p rackscreen-setup`
Expected: green, including any `all_fields_round_trip`-style test that iterates `FIELDS` (the new fields round-trip through `get`/`set`: `"true"`, `"DK1"`, `"21"`).

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: green.

- [ ] **Step 6: Commit**

```bash
git add crates/setup
git commit -m "feat(setup): price fields become enabled, zone and vat %; hint for a dead price zone

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 6: README, example config, gif and spec touch-up

**Files:**
- Modify: `README.md` (screens table row ~42, roles table row ~116, Electricity mode ~144-150, sky note ~163, config example ~214-219, licence ~320)
- Modify: `config.example.yaml:38-43`
- Regenerate: `.github/media/screens/price.gif`
- Modify: `docs/superpowers/specs/2026-09-11-price-level-gauge-design.md` (needle sweep note)

- [ ] **Step 1: config.example.yaml**

Replace the `price:` block:

```yaml
price:
  enabled: true            # Energy-Charts day-ahead prices, no key
  zone: ""                 # Energy-Charts bidding zone, e.g. NL, DE-LU, DK1; derived from electricity.zone when empty
  vat_pct: 21              # added to the raw exchange price
  poll_secs: 900           # minimum 900; Energy-Charts rate-limits bursts
```

Apply the same block in the README's configuration example (around line 214).

- [ ] **Step 2: README text**

Screens table (line ~42): `**\`price\`** — today's price as a level gauge, the needle at the current quarter-hour against the three-day average`.

Roles table (line ~116): `| \`price\` | Energy-Charts | five bands cheap to pricey, the needle at the current quarter-hour, badge the verdict |`.

Electricity mode: replace the paragraph starting "The `price` role is separate" through the `price incl. VAT` paragraph with:

```markdown
The `price` role is separate and needs no token: day-ahead prices come from [Energy-Charts](https://www.energy-charts.info) by Fraunhofer ISE, at quarter-hour resolution for 40-odd European bidding zones. It is on by default; `price zone` is the Energy-Charts bidding zone (`NL`, `BE`, `DE-LU`, `DK1`, `NO1`, `SE3`, ...) and may stay empty when the electricity zone maps to one, which it does for the countries and zones Electricity Maps and Energy-Charts share. With neither, prices stay off and the log says so. `price vat %` (21 by default) is added to the raw exchange price; energy tax and supplier markup are not included by any feed.

The gauge shows the current quarter-hour's price in €/kWh and where it sits against the mean of today and the two days before: below 60 % of it is `V.CHEAP`, below 90 % `CHEAP`, up to 115 % `NORMAL`, up to 140 % `PRICEY`, above that `V.PRICEY`. The needle swings to each new slot, so set the Pi's time zone once: `sudo timedatectl set-timezone Europe/Amsterdam`. Energy-Charts rate-limits bursts, so `price poll secs` is never below 900. **Status** shows a dot per link, including `electricity` and `prices`: green after a successful poll, red after failures, grey while nothing has been logged yet.
```

Sky roles note (line ~163): change "The sun dial and the price ring do use the Pi's own zone" to "The sun dial and the price gauge do use the Pi's own zone".

Licence (line ~320): replace "Grid data from Electricity Maps and day-ahead prices from EnergyZero or ENTSO-E belong to those services and are subject to their own terms." with "Grid data from Electricity Maps belongs to that service and is subject to its terms. Day-ahead prices come from [Energy-Charts](https://www.energy-charts.info) by Fraunhofer ISE under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/)."

Search the README for any other `EnergyZero`, `ENTSO-E`, `entsoe`, `incl. VAT`, `current hour breathing` and fix each (the Setup TUI Home paragraph around line 93 lists `prices` as a link name and stays).

- [ ] **Step 3: Regenerate the gif**

Run: `cargo run --example gifs -p rackscreen-app --features sim -- .github/media/screens`
Expected: a line per clip, `price.gif` among them. Open `.github/media/screens/price.gif` and check: bands, needle moving as the fake curve rotates, `€/kWh` caption, the badge word changing.

If the gifs example takes an output dir differently, use its default (`OUT_DIR` in `gifs.rs`) and copy `price.gif` over. Only `price.gif` should differ; if other gifs changed only by encoder noise, `git checkout` them.

- [ ] **Step 4: Spec touch-up**

In the spec's **Needle** paragraph replace "at angle `225 + 270 * t`" with "at angle `225 + 264 * t`, so `t = 0` and `t = 1` sit on the centres of the first and last segment".

- [ ] **Step 5: Commit**

```bash
git add README.md config.example.yaml .github/media/screens/price.gif docs/superpowers/specs/2026-09-11-price-level-gauge-design.md
git commit -m "docs: price gauge on Energy-Charts, config fields, attribution, new gif

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

## Self-review

- **Spec coverage:** arc/bands/needle/centre/badge/no-data (Task 4 step 3), slot from wall clock (Task 4 step 4), event/model (steps 1-2), source and 429 backoff (step 6), zone table (Task 2) and resolution (step 8), config with old keys (step 7), renderer Text (Task 1), TUI fields and hints (Task 5), README/licence/gif (Task 6), fake source (step 5), tests per spec list (Tasks 2-5, golden in Task 4 step 9).
- **Types:** `Event::Prices { date, eur_per_kwh, avg_eur_per_kwh, currency }` everywhere; `PriceState { date, eur, avg, currency, have }`; `price_level -> (PriceLevel, f32)`; `PriceConfig { zone, vat_pct, poll_secs, tz }`; `PriceCfg { enabled, zone, vat_pct, poll_secs }`.
- **Known deviation from spec:** needle sweep is 264° (segment centre to segment centre), corrected in the spec in Task 6.
