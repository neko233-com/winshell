use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ThemeOverrides {
    pub background: Option<String>,
    pub foreground: Option<String>,
    pub accent: Option<String>,
    pub panel: Option<String>,
    pub border: Option<String>,
    pub selection: Option<String>,
}

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub background: u32,
    pub foreground: u32,
    pub accent: u32,
    pub panel: u32,
    pub raised: u32,
    pub border: u32,
    pub muted: u32,
    pub selected: u32,
    pub selection: u32,
    pub danger: u32,
    pub warning: u32,
    pub ansi: [u32; 16],
}

pub const THEMES: &[(&str, &str)] = &[
    ("midnight", "Midnight"),
    ("catppuccin", "Catppuccin"),
    ("nord", "Nord"),
    ("light", "Light"),
    ("custom", "Custom"),
];

impl Default for Theme {
    fn default() -> Self {
        Self {
            background: 0x101419,
            foreground: 0xd8dee9,
            accent: 0x74d5bb,
            panel: 0x161c24,
            raised: 0x222d39,
            border: 0x303c4b,
            muted: 0x8c9bb0,
            selected: 0x233d35,
            selection: 0x344c5b,
            danger: 0xef7d8e,
            warning: 0xeac48b,
            ansi: [
                0x26303b, 0xef7d8e, 0x91d7a3, 0xeac48b, 0x82aaff, 0xc3a6ff, 0x7dd6df, 0xd8dee9,
                0x657387, 0xff96a7, 0xb4edb5, 0xffdfa8, 0xaac5ff, 0xdcc3ff, 0xa0eef4, 0xf2f5fa,
            ],
        }
    }
}

pub fn parse_color(value: &str) -> anyhow::Result<u32> {
    let value = value.strip_prefix('#').unwrap_or(value);
    anyhow::ensure!(
        value.len() == 6 && value.bytes().all(|b| b.is_ascii_hexdigit()),
        "Theme colors must be #RRGGBB"
    );
    Ok(u32::from_str_radix(value, 16)?)
}

impl Theme {
    pub fn resolve(config: &crate::config::Config) -> Self {
        let mut theme = match config.theme.as_str() {
            "catppuccin" => Self {
                background: 0x1e1e2e,
                foreground: 0xcdd6f4,
                accent: 0xcba6f7,
                panel: 0x181825,
                raised: 0x313244,
                border: 0x45475a,
                muted: 0xa6adc8,
                selected: 0x393149,
                selection: 0x585b70,
                danger: 0xf38ba8,
                warning: 0xf9e2af,
                ansi: [
                    0x45475a, 0xf38ba8, 0xa6e3a1, 0xf9e2af, 0x89b4fa, 0xf5c2e7, 0x94e2d5, 0xbac2de,
                    0x585b70, 0xf38ba8, 0xa6e3a1, 0xf9e2af, 0x89b4fa, 0xf5c2e7, 0x94e2d5, 0xa6adc8,
                ],
            },
            "nord" => Self {
                background: 0x2e3440,
                foreground: 0xe5e9f0,
                accent: 0x88c0d0,
                panel: 0x272d38,
                raised: 0x3b4252,
                border: 0x4c566a,
                muted: 0xa8b3c7,
                selected: 0x3b4e5d,
                selection: 0x4c566a,
                danger: 0xbf616a,
                warning: 0xebcb8b,
                ansi: [
                    0x3b4252, 0xbf616a, 0xa3be8c, 0xebcb8b, 0x81a1c1, 0xb48ead, 0x88c0d0, 0xe5e9f0,
                    0x4c566a, 0xbf616a, 0xa3be8c, 0xebcb8b, 0x81a1c1, 0xb48ead, 0x8fbcbb, 0xeceff4,
                ],
            },
            "light" => Self {
                background: 0xf8fafc,
                foreground: 0x202b3b,
                accent: 0x087f68,
                panel: 0xeef2f6,
                raised: 0xe1e8ef,
                border: 0xc4ceda,
                muted: 0x596b81,
                selected: 0xd5ede5,
                selection: 0xb7d8f3,
                danger: 0xad2245,
                warning: 0x986012,
                ansi: [
                    0x202b3b, 0xad2245, 0x237b41, 0x986012, 0x275eab, 0x824bb1, 0x077986, 0x596b81,
                    0x66778a, 0xcc3154, 0x358b41, 0xb67e15, 0x3074bf, 0x9b57c9, 0x078998, 0x1b2532,
                ],
            },
            _ => Self::default(),
        };
        if config.theme == "custom" {
            let c = &config.custom_theme;
            for (value, destination) in [
                (&c.background, &mut theme.background),
                (&c.foreground, &mut theme.foreground),
                (&c.accent, &mut theme.accent),
                (&c.panel, &mut theme.panel),
                (&c.border, &mut theme.border),
                (&c.selection, &mut theme.selection),
            ] {
                if let Some(value) = value.as_ref().and_then(|value| parse_color(value).ok()) {
                    *destination = value;
                }
            }
        }
        theme
    }

    /// Map the UI's semantic color tokens to the selected skin. ANSI output is
    /// resolved separately so explicit colors from applications remain intact.
    pub fn map(self, token: u32) -> u32 {
        match token {
            0x101419 => self.background,
            0xd8dee9 | 0xdce5f0 | 0xf1f5fb | 0xbfe9dc => self.foreground,
            0x74d5bb | 0x96b7ab => self.accent,
            0x161c24 | 0x161d25 | 0x19202a | 0x131921 => self.panel,
            0x1b232e | 0x202832 | 0x2a3340 | 0x222c38 | 0x293340 => self.raised,
            0x233631 | 0x25312f | 0x192322 | 0x263834 | 0x2a413b => self.selected,
            0x29313c | 0x3a4656 | 0x30413f => self.border,
            0x3f3038 | 0x282329 | 0x3d2830 => self.raised,
            0xef7d8e | 0xffb5c0 => self.danger,
            0xeac48b => self.warning,
            0x102720 => self.background,
            _ => self.muted,
        }
    }

    pub fn rgb(self, token: u32) -> gpui::Rgba {
        gpui::rgb(self.map(token))
    }
    pub fn palette(self, index: usize) -> u32 {
        match index {
            0..=15 => self.ansi[index],
            256 => self.foreground,
            257 => self.background,
            258 => self.accent,
            259..=266 => self.ansi[index - 259],
            _ => crate::terminal::indexed_color(index),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_only_rgb_values() {
        assert_eq!(parse_color("#74d5bb").unwrap(), 0x74d5bb);
        for invalid in ["#xyzxyz", "#fff", "red", "ffffffff"] {
            assert!(parse_color(invalid).is_err());
        }
    }
    #[test]
    fn light_and_dark_defaults_have_readable_contrast() {
        for theme in [
            Theme::default(),
            Theme::resolve(&crate::config::Config {
                theme: "light".into(),
                ..Default::default()
            }),
        ] {
            fn luminance(rgb: u32) -> f64 {
                [16, 8, 0]
                    .into_iter()
                    .zip([0.2126, 0.7152, 0.0722])
                    .map(|(shift, weight)| {
                        let c = f64::from((rgb >> shift) & 255) / 255.;
                        weight
                            * if c <= 0.04045 {
                                c / 12.92
                            } else {
                                ((c + 0.055) / 1.055).powf(2.4)
                            }
                    })
                    .sum()
            }
            let a = luminance(theme.foreground);
            let b = luminance(theme.background);
            assert!((a.max(b) + 0.05) / (a.min(b) + 0.05) > 7.);
        }
    }
}
