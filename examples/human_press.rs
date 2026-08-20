use std::io;
use std::thread;
use std::time::Duration;

use ainput::{HumanProfile, Point, TouchController, TouchDevice, TouchMultiplexer};

fn main() -> io::Result<()> {
    println!("[1] Detecting touchscreen...");

    let device = TouchDevice::detect().map_err(io::Error::other)?;

    println!("[OK] {}", device.summary());

    println!("[2] Creating multiplexer...");

    let multiplexer = TouchMultiplexer::builder()
        .build(device)?;

    let display = multiplexer.display_size();

    let center = Point::new(display.width / 2, display.height / 2);

    let profile = HumanProfile::new()
        .with_down_delay(Duration::from_millis(20))
        .with_up_delay(Duration::from_millis(30));

    let mut touch = TouchController::new(multiplexer, profile);

    println!("[3] DOWN at ({}, {})", center.x, center.y);

    touch.touch_down(center)?;

    println!("[4] Holding for 3 seconds...");

    let deadline = std::time::Instant::now() + Duration::from_secs(3);

    while std::time::Instant::now() < deadline {
        touch.poll()?;

        thread::sleep(Duration::from_millis(5));
    }

    println!("[5] UP");

    touch.touch_up()?;

    println!("[OK] done.");

    Ok(())
}
