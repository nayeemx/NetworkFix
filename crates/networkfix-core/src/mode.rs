use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    Quick,
    Full,
    HotspotOnly,
    NoInternet,
}

impl FromStr for Mode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "quick" => Ok(Mode::Quick),
            "full" => Ok(Mode::Full),
            "hotspot" | "hotspotonly" | "hotspot-only" => Ok(Mode::HotspotOnly),
            "no-internet" | "nointernet" | "no_internet" => Ok(Mode::NoInternet),
            other => Err(format!("unknown mode: {other}")),
        }
    }
}

impl Mode {
    pub fn parse(s: &str) -> Result<Mode, String> {
        s.parse()
    }
}

impl std::fmt::Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Mode::Quick => "quick",
            Mode::Full => "full",
            Mode::HotspotOnly => "hotspot",
            Mode::NoInternet => "no-internet",
        };
        write!(f, "{s}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn parses_mode_aliases() {
        assert_eq!(Mode::from_str("quick"), Ok(Mode::Quick));
        assert_eq!(Mode::from_str("Full"), Ok(Mode::Full));
        assert_eq!(Mode::from_str("hotspot"), Ok(Mode::HotspotOnly));
        assert_eq!(Mode::from_str("no-internet"), Ok(Mode::NoInternet));
        assert!(Mode::from_str("bogus").is_err());
    }
}
