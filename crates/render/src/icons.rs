//! Lucide icons parsed once with usvg, drawn as strokes in any colour, size and scale.

use std::collections::HashMap;

use anyhow::{Context, Result};
use rackscreen_core::theme::Color;
use tiny_skia::{FillRule, LineCap, LineJoin, Path, Pixmap, Stroke, Transform};

use crate::assets::{icon_svg, ICON_NAMES};
use crate::prims::paint;

const ICON_UNITS: f32 = 24.0;

struct IconPaths {
    strokes: Vec<(Path, f32)>,
    fills: Vec<Path>,
}

fn collect(group: &usvg::Group, out: &mut IconPaths) {
    for node in group.children() {
        match node {
            usvg::Node::Group(g) => collect(g, out),
            usvg::Node::Path(p) => {
                let Some(path) = p.data().clone().transform(p.abs_transform()) else {
                    continue;
                };
                if let Some(st) = p.stroke() {
                    out.strokes.push((path.clone(), st.width().get()));
                }
                if p.fill().is_some() {
                    out.fills.push(path);
                }
            }
            _ => {}
        }
    }
}

fn load(svg: &[u8]) -> Result<IconPaths> {
    let tree = usvg::Tree::from_data(svg, &usvg::Options::default())
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let mut out = IconPaths {
        strokes: Vec::new(),
        fills: Vec::new(),
    };
    collect(tree.root(), &mut out);
    anyhow::ensure!(
        !out.strokes.is_empty() || !out.fills.is_empty(),
        "icon has no paths"
    );
    Ok(out)
}

pub struct IconCache {
    icons: HashMap<&'static str, IconPaths>,
}

impl IconCache {
    pub fn new() -> Result<Self> {
        let mut icons = HashMap::new();
        for name in ICON_NAMES {
            let svg = icon_svg(name).context(*name)?;
            icons.insert(*name, load(svg).with_context(|| format!("icon {name}"))?);
        }
        Ok(Self { icons })
    }

    pub fn has(&self, name: &str) -> bool {
        self.icons.contains_key(name)
    }

    /// Draw `name` centred at (cx, cy + dy), `size` px across at scale 1.0.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        px: &mut Pixmap,
        name: &str,
        cx: f32,
        cy: f32,
        size: f32,
        color: Color,
        alpha: f32,
        scale: f32,
        dy: f32,
    ) {
        if alpha <= 0.0 || scale <= 0.0 {
            return;
        }
        let Some(icon) = self.icons.get(name) else {
            return;
        };
        let k = scale * size / ICON_UNITS;
        let ts = Transform::from_translate(cx, cy + dy)
            .pre_scale(k, k)
            .pre_translate(-ICON_UNITS / 2.0, -ICON_UNITS / 2.0);
        let p = paint(color, alpha);
        for (path, width) in &icon.strokes {
            let stroke = Stroke {
                width: *width,
                line_cap: LineCap::Round,
                line_join: LineJoin::Round,
                ..Stroke::default()
            };
            px.stroke_path(path, &p, &stroke, ts, None);
        }
        for path in &icon.fills {
            px.fill_path(path, &p, FillRule::Winding, ts, None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rackscreen_core::theme::WHITE;

    fn lit_bbox(p: &Pixmap) -> Option<(u32, u32, u32, u32)> {
        let w = p.width();
        let mut b: Option<(u32, u32, u32, u32)> = None;
        for y in 0..p.height() {
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                if p.data()[i] > 40 {
                    b = Some(match b {
                        None => (x, y, x, y),
                        Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
                    });
                }
            }
        }
        b
    }

    #[test]
    fn all_icons_load() {
        let c = IconCache::new().unwrap();
        for n in ICON_NAMES {
            assert!(c.has(n), "{n}");
        }
        assert!(!c.has("nope"));
    }

    #[test]
    fn cpu_icon_is_centred_and_sized() {
        let c = IconCache::new().unwrap();
        let mut p = Pixmap::new(240, 240).unwrap();
        p.fill(tiny_skia::Color::BLACK);
        c.draw(&mut p, "cpu", 120.0, 98.0, 72.0, WHITE, 1.0, 1.0, 0.0);
        let (x0, y0, x1, y1) = lit_bbox(&p).unwrap();
        let cx = (x0 + x1) as f32 / 2.0;
        let cy = (y0 + y1) as f32 / 2.0;
        assert!((cx - 120.0).abs() <= 1.5, "cx {cx}");
        assert!((cy - 98.0).abs() <= 1.5, "cy {cy}");
        let w = (x1 - x0) as f32;
        assert!(w > 56.0 && w <= 74.0, "width {w}");
    }

    #[test]
    fn scale_and_dy_apply() {
        let c = IconCache::new().unwrap();
        let mut a = Pixmap::new(240, 240).unwrap();
        a.fill(tiny_skia::Color::BLACK);
        c.draw(&mut a, "box", 120.0, 120.0, 72.0, WHITE, 1.0, 1.0, 0.0);
        let mut b = Pixmap::new(240, 240).unwrap();
        b.fill(tiny_skia::Color::BLACK);
        c.draw(&mut b, "box", 120.0, 120.0, 72.0, WHITE, 1.0, 1.2, 10.0);
        let (ax0, ay0, ax1, _) = lit_bbox(&a).unwrap();
        let (bx0, by0, bx1, _) = lit_bbox(&b).unwrap();
        assert!(bx1 - bx0 > ax1 - ax0, "scaled wider");
        assert!(by0 > ay0, "moved down");
    }
}
