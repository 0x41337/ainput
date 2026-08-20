use std::io;
use std::thread;
use std::time::Duration;

use ainput::{Point, TouchDevice, TouchMultiplexer};

fn main() -> io::Result<()> {
    /*
     * Autodetects the physical touchscreen.
     */
    let device = TouchDevice::detect().map_err(io::Error::other)?;

    println!("Touchscreen: {}", device.name());

    /*
     * Creates the multiplexer.
     */
    let mut touch = TouchMultiplexer::builder()
        .build(device)?;

    let display = touch.display_size();

    /*
     * Logical center of the screen.
     */
    let center = Point::new(display.width / 2, display.height / 2);

    println!(
        "Pressing ({}, {}) for 3 seconds...",
        center.x, center.y
    );

    /*
     * DOWN.
     */
    touch.touch_down(center)?;

    /*
     * Hold pressed.
     *
     * During these 3 seconds, physical contacts
     * are only processed if we call poll(). Therefore, the
     * loop must also keep pumping evdev.
     */
    let deadline = std::time::Instant::now() + Duration::from_secs(3);

    while std::time::Instant::now() < deadline {
        touch.poll()?;

        thread::sleep(Duration::from_millis(5));
    }

    /*
     * UP.
     */
    touch.touch_up()?;

    println!("Touch released.");

    Ok(())
}
