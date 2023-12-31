use std::ops::{Deref, DerefMut, BitOr};
use std::cmp::{min, max};

/// An integer representing a position in a file by byte index.
pub type Pos = usize;
 
/// A range of bytes in a file.
/// Made up of a start and end position (inclusive, exclusive).
#[derive(Clone, Copy, Hash, Debug, PartialEq, Eq)] 
pub struct Range(pub Pos, pub Pos);

impl Range {
    pub fn merge(self, other: Self) -> Self {
        Range(min(self.0, other.0), max(self.1, other.1))
    }

    pub fn extend_start(self, start: Pos) -> Self {
        Range(min(self.0, start), self.1)
    }

    #[allow(dead_code)]
    pub fn extend_end(self, end: Pos) -> Self {
        Range(self.0, max(self.1, end))
    }
}

impl From<std::ops::Range<Pos>> for Range {
    fn from(range: std::ops::Range<Pos>) -> Self {
        Range(range.start, range.end)
    }
}

impl BitOr<Range> for Range {
    type Output = Range;

    fn bitor(self, rhs: Range) -> Self::Output {
        self.merge(rhs)
    }
}

impl Ord for Range {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        if self.0 == other.0 {
            self.1.cmp(&other.1)
        } else {
            self.0.cmp(&other.0)
        }
    }
}

impl PartialOrd for Range {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone, Hash, Debug, PartialEq, Eq)] 
pub struct Located<T>(pub T, pub Range);

impl<T> Located<T> {
    #[allow(dead_code)]
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Located<U> {
        Located(f(self.0), self.1)
    }
}

impl<T> Deref for Located<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for Located<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

pub trait IntoLocated<T>: Into<T> {
    fn into_located(self, range: impl Into<Range>) -> Located<T> {
        Located(self.into(), range.into())
    }
}

impl<T> IntoLocated<T> for T {}
