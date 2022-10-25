# Example of memory leak drawing images when using Metal as a backend.

**Run tests to illustrate issue** The tests illustrate running with no metal backend, and with
a metal backend. When using metal we leak several hundred megs per second, and without it memory
remains completely stable.

cargo test -- --nocapture
