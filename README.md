# Lobo

Lobo is a Rust UI framework inspired by Flutter. It offers a familiar, Flutter-like
syntax while aiming to preserve native performance.

Lobo uses a multithreaded runtime, but widget creation remains single-threaded to
avoid data races. UI APIs are synchronous by default, while the `wgpu` rendering
runtime can perform UI work across multiple threads. `wgpu` provides
cross-platform rendering, and Vello CPU is used for CPU-side computation rather
than GPU computation.

State is managed with testable signals. Signals are thread-local and
single-threaded, preventing concurrent access and data races. When user code
needs concurrency, Lobo abstracts task spawning through Tokio so the widget API
itself can remain single-threaded.
