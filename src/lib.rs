use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

/// Similar to `tokio::time::Delay`,
/// but you can push back the moment when this future will resolve
pub struct PostponableDelay {
    delay: tokio::time::Delay,
    target: Arc<Mutex<(std::time::Instant, bool)>>,
}

impl PostponableDelay {
    /// Returns a future that will resolve no sooner than `instant`
    pub fn new(instant: std::time::Instant) -> Self {
        let target = instant.into();
        PostponableDelay {
            delay: tokio::time::delay_until(target),
            target: Arc::new(Mutex::new((instant, false))),
        }
    }

    /// Returns a handle to allow pushing back the future's resolution
    pub fn get_handle(&self) -> PostponableDelayHandle {
        PostponableDelayHandle {
            target: self.target.clone(),
        }
    }

    fn project(
        &mut self,
    ) -> (
        Pin<&mut tokio::time::Delay>,
        &Mutex<(std::time::Instant, bool)>,
    ) {
        (Pin::new(&mut self.delay), &self.target)
    }
}

/// A handle to postpone a `ResettableDelay`'s resolution
pub struct PostponableDelayHandle {
    target: Arc<Mutex<(std::time::Instant, bool)>>,
}

/// The result of a postpone request
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PostponeDelayResponse {
    /// The delay has been successfully postponed
    Ok,
    /// The delay can't be postponed because it has already resolved
    AlreadyResolved,
    /// The request would need the task to be polled earlier than it could,
    /// and was thus refused
    CantResolveEarlier,
}

impl PostponeDelayResponse {
    pub fn unwrap(self) {
        match self {
            PostponeDelayResponse::Ok => {}
            other => panic!("Called .unwrap() on {:?}", other),
        }
    }
}

impl PostponableDelayHandle {
    /// Attempts to postopone the corresponding `PostponableDelay`'s resolution,
    /// returns a `PostponeDelayResponse` detailing if it succeeded.
    #[must_use]
    pub fn postpone(&self, target: std::time::Instant) -> PostponeDelayResponse {
        let mut guard = self.target.lock().unwrap();
        let previous_target = guard.0;
        if guard.1 {
            PostponeDelayResponse::AlreadyResolved
        } else if target < previous_target {
            PostponeDelayResponse::CantResolveEarlier
        } else {
            *&mut guard.0 = target;
            PostponeDelayResponse::Ok
        }
    }
}

impl std::future::Future for PostponableDelay {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let (delay, target) = self.project();
        match delay.poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(_) => {
                let mut guard = target.lock().unwrap();
                let target = guard.0;
                if target <= std::time::Instant::now() {
                    guard.1 = true;
                    Poll::Ready(())
                } else {
                    std::mem::drop(guard);
                    self.delay = tokio::time::delay_until(target.into());
                    self.poll(cx)
                }
            }
        }
    }
}

#[cfg(test)]
const ERROR_MARGIN: std::time::Duration = std::time::Duration::from_millis(3);

#[tokio::test]
async fn no_resets() {
    let target = std::time::Instant::now() + 4 * ERROR_MARGIN;
    std::thread::sleep(2 * ERROR_MARGIN);
    PostponableDelay::new(target).await;
    let end = std::time::Instant::now();
    println!("{:?}", end - target);
    assert!(target <= end);
    assert!(end <= target + ERROR_MARGIN);
}

#[tokio::test]
async fn with_resets() {
    let target = std::time::Instant::now() + 4 * ERROR_MARGIN;
    let delay = PostponableDelay::new(target);
    let handle = delay.get_handle();
    std::thread::sleep(2 * ERROR_MARGIN);
    let target = std::time::Instant::now() + 4 * ERROR_MARGIN;
    handle.postpone(target).unwrap();
    assert_eq!(
        handle.postpone(target - ERROR_MARGIN),
        PostponeDelayResponse::CantResolveEarlier
    );
    delay.await;
    let end = std::time::Instant::now();
    println!("{:?}", end - target);
    assert_eq!(handle.postpone(end), PostponeDelayResponse::AlreadyResolved);
    assert!(target <= end);
    assert!(end <= target + ERROR_MARGIN);
}
