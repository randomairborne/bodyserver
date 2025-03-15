use std::{convert::Infallible, sync::Arc, task::Poll, time::Duration};

use bytes::Bytes;
use futures_core::Stream;
use tokio::time::{Instant, Interval};

#[derive(Debug, Clone)]
pub struct CyclingBodySource {
    bodies: Arc<[Bytes]>,
    wait: Duration,
}

impl CyclingBodySource {
    pub fn new(bodies: Arc<[Bytes]>, wait: Duration) -> Result<Self, InvalidItemLength> {
        if !bodies.is_empty() {
            Ok(Self { bodies, wait })
        } else {
            Err(InvalidItemLength)
        }
    }
    pub fn from_vecs(bodies: Vec<Vec<u8>>, wait: Duration) -> Result<Self, InvalidItemLength> {
        Self::new(bodies.into_iter().map(Bytes::from_owner).collect(), wait)
    }
}

#[derive(Debug)]
pub struct CyclingBody {
    bodies: Arc<[Bytes]>,
    current: usize,
    interval: Interval,
}

impl From<CyclingBodySource> for CyclingBody {
    fn from(src: CyclingBodySource) -> Self {
        Self {
            bodies: src.bodies,
            current: 0,
            interval: tokio::time::interval_at(Instant::now(), src.wait),
        }
    }
}

#[derive(Debug)]
pub struct InvalidItemLength;

impl std::fmt::Display for InvalidItemLength {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "You must pass in a vec of at least one element")
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
                let minibody = self.bodies[self.current].clone();
                self.current = if self.current == self.bodies.len() - 1 {
                    0
                } else {
                    self.current + 1
                };
                Poll::Ready(Some(Ok(minibody)))
            }
        }
    }
}

impl http_body::Body for CyclingBody {
    type Data = Bytes;
    type Error = std::convert::Infallible;

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
