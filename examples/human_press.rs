use std::io;
use std::thread;
use std::time::Duration;

use ainput::{HumanProfile, Point, TouchController, TouchDevice, TouchMultiplexer};

fn main() -> io::Result<()> {
    println!("[1] Detecting touchscreen...");

    let device = TouchDevice::detect().map_err(io::Error::other)?;

    println!("[OK] {}", device.summary());

    println!("[2] Creating a multiplexer...");

    let multiplexer = TouchMultiplexer::open(device)?;

    let display = multiplexer.display_size();

    let center = Point::new(display.width / 2, display.height / 2);

    /*
     * Creates the human profile.
     *
     * In this initial version, the HumanProfile still
     * preserves the trajectory, but it already passes
     * through the profile layer.
     */
    let profile = HumanProfile::new()
        .with_down_delay(Duration::from_millis(20))
        .with_up_delay(Duration::from_millis(30));

    let mut touch = TouchController::new(multiplexer, profile);

    println!("[3] Pressing the center: ({}, {})", center.x, center.y);

    /*
     * DOWN via the profile.
     */
    touch.touch_down(center)?;

    println!("[4] By holding it down for 3 seconds...");

    /*
     * It is important to continue processing the
     * physical touchscreen during the period.
     */
    let deadline = std::time::Instant::now() + Duration::from_secs(3);

    while std::time::Instant::now() < deadline {
        touch.poll()?;

        thread::sleep(Duration::from_millis(5));
    }

    println!("[5] Releasing...");

    /*
     * UP via profile.
     */
    touch.touch_up()?;

    println!("[OK] Touch complete.");

    Ok(())
}
