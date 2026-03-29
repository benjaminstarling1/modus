// ─────────────────────────────────────────────────────────────────────────────
// Colour palettes
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub enum Palette {
    #[default]
    Viridis,
    Plasma,
    Cool,
    Hot,
    Turbo,
}

impl Palette {
    pub fn label(&self) -> &'static str {
        match self {
            Palette::Viridis => "Viridis",
            Palette::Plasma  => "Plasma",
            Palette::Cool    => "Cool",
            Palette::Hot     => "Hot",
            Palette::Turbo   => "Turbo",
        }
    }

    /// Map a value in [0, 1] to an RGBA colour.
    /// If `reverse` is true the palette direction is flipped.
    pub fn sample(&self, t: f32, reverse: bool) -> [f32; 4] {
        let t = if reverse { 1.0 - t.clamp(0.0, 1.0) } else { t.clamp(0.0, 1.0) };
        let stops: &[[f32; 3]] = match self {
            Palette::Viridis => &[
                [0.267, 0.005, 0.329],
                [0.128, 0.567, 0.551],
                [0.204, 0.788, 0.467],
                [0.769, 0.882, 0.216],
                [0.993, 0.906, 0.144],
            ],
            Palette::Plasma => &[
                [0.050, 0.030, 0.528],
                [0.558, 0.056, 0.654],
                [0.899, 0.219, 0.458],
                [0.980, 0.565, 0.163],
                [0.940, 0.975, 0.131],
            ],
            Palette::Cool => &[
                [0.0,  1.0, 1.0],
                [0.25, 0.75, 1.0],
                [0.5,  0.5, 1.0],
                [0.75, 0.25, 1.0],
                [1.0,  0.0, 1.0],
            ],
            Palette::Hot => &[
                [0.04, 0.0, 0.0],
                [0.6,  0.0, 0.0],
                [1.0,  0.4, 0.0],
                [1.0,  1.0, 0.0],
                [1.0,  1.0, 1.0],
            ],
            Palette::Turbo => &[
                [0.190, 0.072, 0.232],
                [0.065, 0.365, 0.860],
                [0.120, 0.724, 0.830],
                [0.450, 0.978, 0.347],
                [0.890, 0.860, 0.140],
                [0.976, 0.533, 0.084],
                [0.761, 0.028, 0.051],
            ],
        };

        let n    = stops.len() - 1;
        let idx  = (t * n as f32).floor() as usize;
        let idx  = idx.min(n - 1);
        let frac = t * n as f32 - idx as f32;
        let a    = stops[idx];
        let b    = stops[idx + 1];
        [
            a[0] + (b[0] - a[0]) * frac,
            a[1] + (b[1] - a[1]) * frac,
            a[2] + (b[2] - a[2]) * frac,
            1.0,
        ]
    }
}
