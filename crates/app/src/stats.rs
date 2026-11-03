//! Per-screen frame timings, logged once per period so the Pi shows where
//! the frame budget goes.

use rackscreen_core::anim::Secs;
use rackscreen_display::PushStats;

/// Millisecond samples over one logging period.
#[derive(Clone, Debug, Default)]
pub struct Window {
    samples: Vec<f32>,
}

impl Window {
    pub fn push(&mut self, ms: f32) {
        self.samples.push(ms);
    }
    pub fn len(&self) -> usize {
        self.samples.len()
    }
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }
    pub fn avg(&self) -> f32 {
        if self.samples.is_empty() {
            return 0.0;
        }
        self.samples.iter().sum::<f32>() / self.samples.len() as f32
    }
    /// Nearest-rank 95th percentile.
    pub fn p95(&self) -> f32 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let mut v = self.samples.clone();
        v.sort_by(|a, b| a.total_cmp(b));
        let rank = ((0.95 * v.len() as f32).ceil() as usize).clamp(1, v.len());
        v[rank - 1]
    }
    pub fn clear(&mut self) {
        self.samples.clear();
    }
}

#[derive(Clone, Debug, Default)]
pub struct ScreenStats {
    pub build: Window,
    pub render: Window,
    pub flush: Window,
    pub skipped: u32,
}

pub struct FrameStats {
    screens: Vec<ScreenStats>,
    period: Secs,
    since: Secs,
}

impl FrameStats {
    pub fn new(n: usize, period: Secs, now: Secs) -> Self {
        Self {
            screens: vec![ScreenStats::default(); n],
            period,
            since: now,
        }
    }
    pub fn screen(&mut self, i: usize) -> &mut ScreenStats {
        &mut self.screens[i]
    }
    pub fn due(&self, now: Secs) -> bool {
        now - self.since >= self.period
    }
    /// Seconds since the period started (construction or the last `reset`).
    pub fn elapsed(&self, now: Secs) -> f64 {
        now - self.since
    }
    /// One log line for screen `i`; `secs` is the length of the period.
    pub fn line(&self, i: usize, push: PushStats, secs: f64) -> String {
        let s = &self.screens[i];
        let push_avg = if push.frames > 0 {
            push.total_ms / push.frames as f64
        } else {
            0.0
        };
        let kb = if push.frames > 0 {
            push.bytes as f64 / push.frames as f64 / 1000.0
        } else {
            0.0
        };
        format!(
            "frame stats screen={i} fps={:.1} rendered={} skipped={} build_ms={:.2}/{:.2} \
             render_ms={:.2}/{:.2} flush_ms={:.2}/{:.2} push_ms={push_avg:.2}/{:.2} kb_per_frame={kb:.1}",
            push.frames as f64 / secs.max(1e-9),
            s.render.len(),
            s.skipped,
            s.build.avg(),
            s.build.p95(),
            s.render.avg(),
            s.render.p95(),
            s.flush.avg(),
            s.flush.p95(),
            push.max_ms,
        )
    }
    pub fn reset(&mut self, now: Secs) {
        for s in &mut self.screens {
            *s = ScreenStats::default();
        }
        self.since = now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_avg_and_p95() {
        let mut w = Window::default();
        for i in 1..=100 {
            w.push(i as f32);
        }
        assert!((w.avg() - 50.5).abs() < 1e-3);
        assert_eq!(w.p95(), 95.0);
        w.clear();
        assert_eq!(w.avg(), 0.0);
        assert_eq!(w.p95(), 0.0);
    }

    #[test]
    fn due_after_period_and_reset() {
        let mut s = FrameStats::new(2, 60.0, 0.0);
        assert!(!s.due(59.9));
        assert!(s.due(60.0));
        s.screen(1).render.push(2.0);
        s.reset(60.0);
        assert!(!s.due(100.0));
        assert_eq!(s.screen(1).render.avg(), 0.0);
    }

    #[test]
    fn elapsed_follows_reset() {
        let mut s = FrameStats::new(1, 60.0, 0.0);
        assert_eq!(s.elapsed(45.0), 45.0);
        s.reset(100.0);
        assert_eq!(s.elapsed(130.0), 30.0);
    }

    #[test]
    fn line_reports_fps_and_timings() {
        let mut s = FrameStats::new(1, 60.0, 0.0);
        s.screen(0).build.push(0.2);
        s.screen(0).render.push(1.5);
        s.screen(0).flush.push(0.5);
        s.screen(0).skipped = 3;
        let push = PushStats {
            frames: 1800,
            total_ms: 3600.0,
            max_ms: 9.0,
            bytes: 1800 * 1000,
        };
        assert_eq!(
            s.line(0, push, 60.0),
            "frame stats screen=0 fps=30.0 rendered=1 skipped=3 build_ms=0.20/0.20 \
             render_ms=1.50/1.50 flush_ms=0.50/0.50 push_ms=2.00/9.00 kb_per_frame=1.0"
        );
    }
}
