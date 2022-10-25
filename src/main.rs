pub mod canvas;
use canvas::Canvas;
const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;

fn main() {
    let _canvas = Canvas::new(WIDTH, HEIGHT);
}

#[cfg(test)]
mod tests {
    use super::canvas::Canvas;
    use crate::HEIGHT;
    use crate::WIDTH;
    use once_cell::sync::Lazy;
    use serial_test::serial;
    use std::time::Duration;

    const MS_PER_FRAME: Duration = std::time::Duration::from_millis(1000 / 30);

    // Fail if we leak 500 MB while capturing.
    const MEMORY_LEAK_THRESHOLD: f64 = 500.0;

    fn get_virtual_memory_raw() -> usize {
        let memory = memory_stats::memory_stats();
        if let Some(usage) = memory {
            usage.virtual_mem as usize
        } else {
            0
        }
    }

    static INITIAL_MEMORY: Lazy<usize> = Lazy::new(|| get_virtual_memory_raw());

    fn get_virtual_memory_mb() -> f64 {
        let initial_memory = *INITIAL_MEMORY;
        let memory = get_virtual_memory_raw();
        memory as f64 / 1024.0 / 1024.0 - initial_memory as f64 / 1024.0 / 1024.0
    }

    fn draw_for_time(canvas: &mut Canvas, t_begin: std::time::Instant) {
        const SECONDS_TO_RUN: std::time::Duration = std::time::Duration::from_millis(5000);
        let mut t_last_memory_profile = std::time::Instant::now();
        let memory_base = get_virtual_memory_mb();
        let mut memory_increase: Option<f64> = None;
        let pixels = vec![0; WIDTH as usize * HEIGHT as usize * 4];
        loop {
            let t_start = std::time::Instant::now();
            canvas.draw_raw_rgb_scale(0, 0, WIDTH, HEIGHT, 1, &pixels);
            let t_frame_elapsed = std::time::Instant::now() - t_start;
            if MS_PER_FRAME > t_frame_elapsed {
                let t_sleep = MS_PER_FRAME - t_frame_elapsed;
                std::thread::sleep(t_sleep);
            }

            let time_since_memory_profile = std::time::Instant::now() - t_last_memory_profile;
            if time_since_memory_profile > std::time::Duration::from_millis(1000) {
                let vmem_increase = get_virtual_memory_mb() - memory_base;
                if let Some(last_memory_increase) = memory_increase {
                    // Fail test, a memory leak has been detected!
                    assert!((vmem_increase - last_memory_increase) < MEMORY_LEAK_THRESHOLD);
                } else {
                    memory_increase = Some(vmem_increase);
                }
                println!(
                    "Memory Usage: {} MB, diff: {}",
                    get_virtual_memory_mb(),
                    vmem_increase
                );
                t_last_memory_profile = std::time::Instant::now();
            }

            let total_time_elapsed = std::time::Instant::now() - t_begin;
            if total_time_elapsed > SECONDS_TO_RUN {
                break;
            }
        }
    }
    #[test]
    #[serial]
    fn test_canvas() {
        let mut canvas = Canvas::new(WIDTH, HEIGHT);
        let t_begin = std::time::Instant::now();
        draw_for_time(&mut canvas, t_begin);
    }

    #[test]
    #[serial]
    fn test_metal_canvas() {
        let canvas = Canvas::new_metal(WIDTH, HEIGHT);
        let t_begin = std::time::Instant::now();
        match canvas {
            Some(mut canvas) => draw_for_time(&mut canvas, t_begin),
            _ => panic!("Expected metal canvas"),
        }
    }
}
