use core::fmt;

/// Recoverable error returned across the Bulldog Complements core pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreError {
    /// Image dimensions are zero, negative, or exceed safe bounds.
    InvalidDimensions { width: u32, height: u32 },
    /// Host row stride is smaller than a pixel row or overflows address arithmetic.
    InvalidStride {
        row_bytes: isize,
        min_row_bytes: usize,
    },
    /// Provided host byte slice is smaller than required by bounds and stride.
    BufferTooSmall { required: usize, actual: usize },
    /// Allocation request exceeded configured safety cap or system memory.
    AllocationLimitExceeded {
        requested_pixels: usize,
        max_pixels: usize,
    },
    /// A parameter descriptor or value failed validation.
    InvalidParameter(String),
    /// A required input clip was missing for the active context.
    MissingInputClip(&'static str),
    /// Rendering was cooperatively cancelled by the host (`OfxAbort`).
    Aborted,
    /// Effect descriptor failed metadata integrity checks.
    InvalidDescriptor(String),
    /// Non-fatal internal processing failure.
    Internal(String),
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDimensions { width, height } => {
                write!(f, "invalid frame dimensions: {width}x{height}")
            }
            Self::InvalidStride {
                row_bytes,
                min_row_bytes,
            } => write!(
                f,
                "invalid row stride {row_bytes} bytes (minimum row size: {min_row_bytes} bytes)"
            ),
            Self::BufferTooSmall { required, actual } => {
                write!(
                    f,
                    "host buffer too small: need {required} bytes, got {actual}"
                )
            }
            Self::AllocationLimitExceeded {
                requested_pixels,
                max_pixels,
            } => write!(
                f,
                "allocation limit exceeded: requested {requested_pixels} pixels (max {max_pixels})"
            ),
            Self::InvalidParameter(msg) => write!(f, "invalid parameter: {msg}"),
            Self::MissingInputClip(name) => write!(f, "missing required input clip: {name}"),
            Self::Aborted => write!(f, "render aborted by host"),
            Self::InvalidDescriptor(msg) => write!(f, "invalid descriptor: {msg}"),
            Self::Internal(msg) => write!(f, "internal error: {msg}"),
        }
    }
}

impl std::error::Error for CoreError {}

/// Standard fallible result type across `bulldog_core`.
pub type CoreResult<T> = Result<T, CoreError>;
