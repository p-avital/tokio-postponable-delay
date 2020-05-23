# A tokio Delay that you can push back
## How does it work?
Like a normal `tokio::time::Delay`, except it will check whether its target time of resolution has moved before resolving, and reschedule itself if needed.

For now, this works by dropping the previous `Delay` and instantiating a new one to reschedule itself.
## How do I use it?
```rust
async fn typical_usage() {
    let target = std::time::Instant::now() + std::time::Duration::from_millis(10);
    let delay = ResettableDelay::new(target);
    let handle = delay.get_handle();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(5));
        let target = std::time::Instant::now() + std::time::Duration::from_millis(10);
        handle.postpone(target)
    });
    delay.await;
}
```
