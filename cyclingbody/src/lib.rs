use std::{convert::Infallible, num::NonZero, sync::Arc, task::Poll, time::Duration};

use bytes::Bytes;
use futures_core::Stream;
use http_body::SizeHint;
use tokio::time::{Instant, Interval};

#[cfg(test)]
mod tests;

#[derive(Debug, Clone)]
pub struct CyclingBodySource {
    bodies: Arc<[Bytes]>,
    wait: Duration,
    loop_limit: Option<NonZero<usize>>,
    bodies_size_sum: usize,
}

impl CyclingBodySource {
    pub fn new(
        bodies: Arc<[Bytes]>,
        wait: Duration,
        loop_limit: Option<NonZero<usize>>,
    ) -> Result<Self, InvalidItemLength> {
        if !bodies.is_empty() {
            Ok(Self {
                bodies_size_sum: bodies.iter().map(Bytes::len).sum(),
                bodies,
                wait,
                loop_limit,
            })
        } else {
            Err(InvalidItemLength)
        }
    }
    pub fn from_vecs(
        bodies: Vec<Vec<u8>>,
        wait: Duration,
        loop_limit: Option<NonZero<usize>>,
    ) -> Result<Self, InvalidItemLength> {
        Self::new(
            bodies.into_iter().map(Bytes::from_owner).collect(),
            wait,
            loop_limit,
        )
    }
}

#[derive(Debug)]
pub struct CyclingBody {
    bodies: Arc<[Bytes]>,
    bodies_size_sum: usize,
    current: usize,
    loop_limit: Option<NonZero<usize>>,
    loop_count: usize,
    interval: Interval,
}

impl From<CyclingBodySource> for CyclingBody {
    fn from(src: CyclingBodySource) -> Self {
        Self {
            bodies: src.bodies,
            bodies_size_sum: src.bodies_size_sum,
            current: 0,
            loop_limit: src.loop_limit,
            loop_count: 0,
            interval: tokio::time::interval_at(Instant::now(), src.wait),
        }
    }
}

#[derive(Debug)]
pub struct InvalidItemLength;

impl std::fmt::Display for InvalidItemLength {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "At least one frame required")
    }
}

impl std::error::Error for InvalidItemLength {}

impl Stream for CyclingBody {
    type Item = Result<Bytes, Infallible>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        match self.interval.poll_tick(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(_) => {
                if self.loop_limit.is_some_and(|v| self.loop_count >= v.get()) {
                    Poll::Ready(None)
                } else {
                    let minibody = self.bodies[self.current].clone();
                    self.current = if self.current == self.bodies.len() - 1 {
                        self.loop_count += 1;
                        0
                    } else {
                        self.current + 1
                    };
                    Poll::Ready(Some(Ok(minibody)))
                }
            }
        }
    }
}

impl http_body::Body for CyclingBody {
    type Data = Bytes;
    type Error = std::convert::Infallible;

    fn size_hint(&self) -> SizeHint {
        if let Some(loop_limit) = self.loop_limit {
            let total_size = self.bodies_size_sum * loop_limit.get();
            // returns a known, exact value only if a usize fits into a u64- otherwise
            // returns the default
            // this code will likely never trigger the sad path
            // it gets optimized to like 4 mov
            total_size
                .try_into()
                .ok()
                .map(SizeHint::with_exact)
                .unwrap_or_else(SizeHint::new)
        } else {
            SizeHint::new()
        }
    }

    fn is_end_stream(&self) -> bool {
        false
    }

    fn poll_frame(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
        self.poll_next(cx)
            .map(|o| o.map(|r| r.map(http_body::Frame::data)))
    }
}
