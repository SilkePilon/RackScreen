# Remove the price and carbon screens

Date: 2026-11-03. Status: approved by Silke ("everything behind them").

## Price

- Delete `Role::Price`, its builder (`p2/roles/energy.rs` `price`), its no-data and gate arms, goldens and README gallery entry.
- Delete the Energy-Charts price poller (`crates/sources/src/prices.rs`), its event and model state, `PriceCfg` / the `price:` config block, the price group/fields in the setup TUI, fake-source price data, and `energy_charts_zone_for` if nothing else uses it.
- An old config's `price:` block is ignored with one `warn` ("the `price:` block is no longer used and is ignored"), like the legacy `face:` block.

## Carbon

- Delete `Role::Carbon`, its builder (`energy.rs` `carbon`), gates, goldens, gif and README entry, and the carbon-only model state (`carbon_hist`, `carbon_hourly`, and `carbon_gco2` from the electricity event and source if nothing else reads it).
- The Electricity Maps source stays: power-mix and renewable use it. The electricity token and its config stay.

## Old roles lists

`price` and `carbon` in a screen's `roles` list are dropped at config load with one `warn` per entry kind, exactly like `face` (same code path in `normalize`); a screen left empty is the same 1-based "screen N has no roles left …" error, naming the removed role(s).

## Testing

- Config: `price`/`carbon` dropped from a roles list; only-removed-roles list errors; legacy `price:` block ignored.
- `Role::ALL` drops to 28. Remaining goldens byte-identical (exact compare); price/carbon goldens deleted. Gifs: their gifs deleted, `all.gif` regenerated.
