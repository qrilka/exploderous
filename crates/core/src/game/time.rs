use std::{fmt, num::NonZeroU64};

/// An absolute simulation tick label, independent of wall-clock time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Tick(u64);

impl Tick {
    pub const ZERO: Self = Self(0);

    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }

    /// Reject an unrepresentable deadline instead of wrapping or shortening it.
    pub fn checked_after(self, span: TickSpan) -> Option<Self> {
        self.0.checked_add(span.value()).map(Self)
    }
}

impl fmt::Display for Tick {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// A positive duration measured in simulation ticks, not an absolute deadline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TickSpan(NonZeroU64);

impl TickSpan {
    pub const ONE: Self = Self(NonZeroU64::MIN);

    pub const fn new(value: u64) -> Option<Self> {
        match NonZeroU64::new(value) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    pub const fn value(self) -> u64 {
        self.0.get()
    }
}
