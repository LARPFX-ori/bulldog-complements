use std::collections::BTreeMap;

use crate::error::{CoreError, CoreResult};
use crate::math::{safe_clamp_f64, sanitize_f64};

/// Parameter kind and bounds advertised to OpenFX hosts.
#[derive(Debug, Clone, PartialEq)]
pub enum ParamKind {
    /// Scalar floating-point parameter with hard and UI display ranges.
    Double {
        default: f64,
        min: f64,
        max: f64,
        display_min: f64,
        display_max: f64,
    },
    /// Scalar integer parameter.
    Int { default: i32, min: i32, max: i32 },
    /// Boolean toggle parameter.
    Bool { default: bool },
    /// Enumerated drop-down choice parameter.
    Choice {
        default: usize,
        options: Vec<String>,
    },
    /// RGBA color picker parameter (channels in `0.0..=1.0`).
    Rgba { default: [f64; 4] },
    /// Normalized 2D coordinate parameter (`0.0..=1.0` resolution-independent).
    Point2D {
        default: [f64; 2],
        min: [f64; 2],
        max: [f64; 2],
    },
}

/// Metadata describing a single effect parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct ParamDescriptor {
    pub name: String,
    pub label: String,
    pub hint: String,
    pub kind: ParamKind,
}

impl ParamDescriptor {
    /// Creates a floating-point parameter descriptor.
    #[must_use]
    pub fn double(
        name: impl Into<String>,
        label: impl Into<String>,
        hint: impl Into<String>,
        default: f64,
        min: f64,
        max: f64,
    ) -> Self {
        Self {
            name: name.into(),
            label: label.into(),
            hint: hint.into(),
            kind: ParamKind::Double {
                default,
                min,
                max,
                display_min: min,
                display_max: max,
            },
        }
    }

    /// Creates an integer parameter descriptor.
    #[must_use]
    pub fn int(
        name: impl Into<String>,
        label: impl Into<String>,
        hint: impl Into<String>,
        default: i32,
        min: i32,
        max: i32,
    ) -> Self {
        Self {
            name: name.into(),
            label: label.into(),
            hint: hint.into(),
            kind: ParamKind::Int { default, min, max },
        }
    }

    /// Creates a boolean parameter descriptor.
    #[must_use]
    pub fn boolean(
        name: impl Into<String>,
        label: impl Into<String>,
        hint: impl Into<String>,
        default: bool,
    ) -> Self {
        Self {
            name: name.into(),
            label: label.into(),
            hint: hint.into(),
            kind: ParamKind::Bool { default },
        }
    }

    /// Creates an enumerated choice parameter descriptor.
    #[must_use]
    pub fn choice(
        name: impl Into<String>,
        label: impl Into<String>,
        hint: impl Into<String>,
        default: usize,
        options: &[&str],
    ) -> Self {
        Self {
            name: name.into(),
            label: label.into(),
            hint: hint.into(),
            kind: ParamKind::Choice {
                default,
                options: options.iter().map(|s| (*s).to_owned()).collect(),
            },
        }
    }

    /// Creates an RGBA color parameter descriptor.
    #[must_use]
    pub fn rgba(
        name: impl Into<String>,
        label: impl Into<String>,
        hint: impl Into<String>,
        default: [f64; 4],
    ) -> Self {
        Self {
            name: name.into(),
            label: label.into(),
            hint: hint.into(),
            kind: ParamKind::Rgba { default },
        }
    }

    /// Creates a normalized 2D point parameter descriptor.
    #[must_use]
    pub fn point2d(
        name: impl Into<String>,
        label: impl Into<String>,
        hint: impl Into<String>,
        default: [f64; 2],
        min: [f64; 2],
        max: [f64; 2],
    ) -> Self {
        Self {
            name: name.into(),
            label: label.into(),
            hint: hint.into(),
            kind: ParamKind::Point2D { default, min, max },
        }
    }

    /// Returns the default [`ParamValue`] corresponding to this descriptor.
    #[must_use]
    pub fn default_value(&self) -> ParamValue {
        match &self.kind {
            ParamKind::Double { default, .. } => ParamValue::Double(*default),
            ParamKind::Int { default, .. } => ParamValue::Int(*default),
            ParamKind::Bool { default } => ParamValue::Bool(*default),
            ParamKind::Choice { default, .. } => ParamValue::Choice(*default),
            ParamKind::Rgba { default } => ParamValue::Rgba(*default),
            ParamKind::Point2D { default, .. } => ParamValue::Point2D(*default),
        }
    }

    /// Validates parameter descriptor invariants (`min <= default <= max`, non-empty labels, finite bounds).
    pub fn validate(&self) -> CoreResult<()> {
        if self.name.trim().is_empty() {
            return Err(CoreError::InvalidParameter(
                "parameter name cannot be empty".to_owned(),
            ));
        }
        if self.label.trim().is_empty() {
            return Err(CoreError::InvalidParameter(format!(
                "parameter '{}' label cannot be empty",
                self.name
            )));
        }
        match &self.kind {
            ParamKind::Double {
                default,
                min,
                max,
                display_min,
                display_max,
            } => {
                if !default.is_finite()
                    || !min.is_finite()
                    || !max.is_finite()
                    || !display_min.is_finite()
                    || !display_max.is_finite()
                {
                    return Err(CoreError::InvalidParameter(format!(
                        "parameter '{}' contains non-finite bounds or default",
                        self.name
                    )));
                }
                if *min > *default || *default > *max {
                    return Err(CoreError::InvalidParameter(format!(
                        "parameter '{}' violates min ({min}) <= default ({default}) <= max ({max})",
                        self.name
                    )));
                }
                if *display_min > *display_max || *display_min < *min || *display_max > *max {
                    return Err(CoreError::InvalidParameter(format!(
                        "parameter '{}' has invalid display range [{display_min}, {display_max}]",
                        self.name
                    )));
                }
            }
            ParamKind::Int { default, min, max } => {
                if *min > *default || *default > *max {
                    return Err(CoreError::InvalidParameter(format!(
                        "parameter '{}' violates min ({min}) <= default ({default}) <= max ({max})",
                        self.name
                    )));
                }
            }
            ParamKind::Bool { .. } => {}
            ParamKind::Choice { default, options } => {
                if options.is_empty() {
                    return Err(CoreError::InvalidParameter(format!(
                        "choice parameter '{}' must declare at least one option",
                        self.name
                    )));
                }
                if *default >= options.len() {
                    return Err(CoreError::InvalidParameter(format!(
                        "choice parameter '{}' default index {default} out of range ({})",
                        self.name,
                        options.len()
                    )));
                }
                if options.iter().any(|opt| opt.trim().is_empty()) {
                    return Err(CoreError::InvalidParameter(format!(
                        "choice parameter '{}' contains an empty option label",
                        self.name
                    )));
                }
            }
            ParamKind::Rgba { default } => {
                if default
                    .iter()
                    .any(|c| !c.is_finite() || !(0.0..=1.0).contains(c))
                {
                    return Err(CoreError::InvalidParameter(format!(
                        "rgba parameter '{}' default channels must be finite in [0.0, 1.0]",
                        self.name
                    )));
                }
            }
            ParamKind::Point2D { default, min, max } => {
                for axis in 0..2 {
                    let Some(&d) = default.get(axis) else {
                        continue;
                    };
                    let Some(&lo) = min.get(axis) else {
                        continue;
                    };
                    let Some(&hi) = max.get(axis) else {
                        continue;
                    };
                    if !d.is_finite() || !lo.is_finite() || !hi.is_finite() || lo > d || d > hi {
                        return Err(CoreError::InvalidParameter(format!(
                            "point2d parameter '{}' axis {axis} violates min <= default <= max",
                            self.name
                        )));
                    }
                }
            }
        }
        Ok(())
    }
}

/// Concrete runtime value evaluated for a parameter at a specific frame time.
#[derive(Debug, Clone, PartialEq)]
pub enum ParamValue {
    Double(f64),
    Int(i32),
    Bool(bool),
    Choice(usize),
    Rgba([f64; 4]),
    Point2D([f64; 2]),
}

/// Evaluated parameter map passed into effect methods.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParamSet {
    values: BTreeMap<String, ParamValue>,
}

impl ParamSet {
    /// Creates an empty parameter set.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            values: BTreeMap::new(),
        }
    }

    /// Populates a parameter set with the defaults declared by `descriptors`.
    #[must_use]
    pub fn defaults_from(descriptors: &[ParamDescriptor]) -> Self {
        let mut set = Self::new();
        for desc in descriptors {
            set.insert(desc.name.clone(), desc.default_value());
        }
        set
    }

    /// Inserts or updates a parameter value.
    pub fn insert(&mut self, name: impl Into<String>, value: ParamValue) {
        self.values.insert(name.into(), value);
    }

    /// Reads a raw parameter value.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&ParamValue> {
        self.values.get(name)
    }

    /// Reads a sanitized floating-point parameter clamped to `[min, max]`.
    #[must_use]
    pub fn get_double(&self, name: &str, default: f64, min: f64, max: f64) -> f64 {
        let raw = match self.values.get(name) {
            Some(ParamValue::Double(v)) => sanitize_f64(*v, default),
            Some(ParamValue::Int(v)) => f64::from(*v),
            _ => default,
        };
        safe_clamp_f64(raw, min, max)
    }

    /// Reads an integer parameter clamped to `[min, max]`.
    #[must_use]
    pub fn get_int(&self, name: &str, default: i32, min: i32, max: i32) -> i32 {
        let (lo, hi) = if min <= max { (min, max) } else { (max, min) };
        let raw = match self.values.get(name) {
            Some(ParamValue::Int(v)) => *v,
            Some(ParamValue::Double(v)) => safe_clamp_f64(
                sanitize_f64(*v, f64::from(default)).round(),
                f64::from(lo),
                f64::from(hi),
            ) as i32,
            _ => default,
        };
        raw.clamp(lo, hi)
    }

    /// Reads a boolean parameter.
    #[must_use]
    pub fn get_bool(&self, name: &str, default: bool) -> bool {
        match self.values.get(name) {
            Some(ParamValue::Bool(v)) => *v,
            _ => default,
        }
    }

    /// Reads an enumerated choice parameter bounded by `max_exclusive`.
    #[must_use]
    pub fn get_choice(&self, name: &str, default: usize, max_exclusive: usize) -> usize {
        if max_exclusive == 0 {
            return 0;
        }
        let raw = match self.values.get(name) {
            Some(ParamValue::Choice(v)) => *v,
            Some(ParamValue::Int(v)) => usize::try_from(*v).unwrap_or(default),
            _ => default,
        };
        if raw < max_exclusive {
            raw
        } else {
            default.min(max_exclusive - 1)
        }
    }

    /// Reads an RGBA color parameter with sanitized channels in `[0.0, 1.0]`.
    #[must_use]
    pub fn get_rgba(&self, name: &str, default: [f64; 4]) -> [f32; 4] {
        let raw = match self.values.get(name) {
            Some(ParamValue::Rgba(v)) => *v,
            _ => default,
        };
        [
            safe_clamp_f64(raw[0], 0.0, 1.0) as f32,
            safe_clamp_f64(raw[1], 0.0, 1.0) as f32,
            safe_clamp_f64(raw[2], 0.0, 1.0) as f32,
            safe_clamp_f64(raw[3], 0.0, 1.0) as f32,
        ]
    }

    /// Reads a 2D point parameter clamped per axis.
    #[must_use]
    pub fn get_point2d(
        &self,
        name: &str,
        default: [f64; 2],
        min: [f64; 2],
        max: [f64; 2],
    ) -> [f32; 2] {
        let raw = match self.values.get(name) {
            Some(ParamValue::Point2D(v)) => *v,
            _ => default,
        };
        [
            safe_clamp_f64(raw[0], min[0], max[0]) as f32,
            safe_clamp_f64(raw[1], min[1], max[1]) as f32,
        ]
    }
}
