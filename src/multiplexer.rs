use std::io;
use std::time::Instant;

use evdev::{Device, EventType};

use crate::{OutputSlot, TouchDevice, VirtualTouchscreen};

const ABS_MT_SLOT_CODE: u16 = 47;
const ABS_MT_TOUCH_MAJOR_CODE: u16 = 48;
const ABS_MT_WIDTH_MAJOR_CODE: u16 = 50;
const ABS_MT_POSITION_X_CODE: u16 = 53;
const ABS_MT_POSITION_Y_CODE: u16 = 54;
const ABS_MT_TRACKING_ID_CODE: u16 = 57;

const SYN_REPORT_CODE: u16 = 0;

/// Logical dimension of the Android display.
///
/// The physical touchscreen has its own raw range.
/// The multiplexer converts logical coordinates to that range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DisplaySize {
    pub width: i32,
    pub height: i32,
}

impl DisplaySize {
    pub const fn new(width: i32, height: i32) -> Self {
        Self { width, height }
    }
}

impl Default for DisplaySize {
    fn default() -> Self {
        Self {
            width: 720,
            height: 1600,
        }
    }
}

/// Logical coordinate on the display.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

/// State of a physical contact.
#[derive(Clone, Copy, Debug, Default)]
struct SlotState {
    active: bool,
    tracking_id: Option<i32>,

    x: i32,
    y: i32,

    touch_major: i32,
    width_major: i32,
}

/// Multiplexes physical and virtual contacts on a single
/// `uinput` touchscreen device.
///
/// The physical device is captured with `EVIOCGRAB`. Its contacts
/// continue to be reproduced on the virtual device, to which
/// a virtual contact can also be added.
pub struct TouchMultiplexer {
    device: Device,
    touchscreen: TouchDevice,
    output: VirtualTouchscreen,

    display_size: DisplaySize,

    current_physical_slot: usize,
    physical_slots: Vec<SlotState>,

    virtual_down: bool,
    virtual_tracking_id: Option<i32>,

    virtual_position: Point,

    virtual_touch_major: i32,
    virtual_width_major: i32,

    last_event_time: Option<Instant>,
}

impl TouchMultiplexer {
    /// Opens a multiplexer using the default `720x1600` dimension.
    pub fn open(touchscreen: TouchDevice) -> io::Result<Self> {
        Self::open_with_display_size(touchscreen, DisplaySize::default())
    }

    /// Opens the multiplexer with an explicitly provided
    /// logical display dimension.
    pub fn open_with_display_size(
        touchscreen: TouchDevice,
        display_size: DisplaySize,
    ) -> io::Result<Self> {
        if display_size.width <= 0 || display_size.height <= 0 {
            return Err(io::Error::other("invalid display dimension"));
        }

        let physical_slot_count = touchscreen.slot_count();

        if physical_slot_count == 0 {
            return Err(io::Error::other("touchscreen without valid slots"));
        }

        /*
         * Opens the physical device.
         */
        let mut device = touchscreen.open().map_err(io::Error::other)?;

        /*
         * Non-blocking so the consumer loop can process
         * both evdev and its own UI.
         */
        device.set_nonblocking(true)?;

        /*
         * Exclusive grab.
         */
        device.grab()?;

        /*
         * Creates the virtual touchscreen.
         */
        let output = match VirtualTouchscreen::create(&touchscreen) {
            Ok(output) => output,

            Err(error) => {
                let _ = device.ungrab();

                return Err(io::Error::other(error));
            }
        };

        Ok(Self {
            device,
            touchscreen,

            output,

            display_size,

            current_physical_slot: 0,

            physical_slots: vec![SlotState::default(); physical_slot_count],

            virtual_down: false,
            virtual_tracking_id: None,

            virtual_position: Point::new(display_size.width / 2, display_size.height / 2),

            virtual_touch_major: 1,
            virtual_width_major: 1,

            last_event_time: None,
        })
    }

    /// Physical touchscreen description.
    pub fn touchscreen(&self) -> &TouchDevice {
        &self.touchscreen
    }

    /// Logical display dimension.
    pub fn display_size(&self) -> DisplaySize {
        self.display_size
    }

    /// Number of physical slots.
    pub fn physical_slot_count(&self) -> usize {
        self.physical_slots.len()
    }

    /// Slot reserved for the virtual touch.
    pub fn virtual_slot(&self) -> usize {
        self.physical_slots.len()
    }

    /// Total number of slots on the virtual device.
    pub fn total_slot_count(&self) -> usize {
        self.physical_slots.len() + 1
    }

    /// Number of active physical contacts.
    pub fn physical_contact_count(&self) -> usize {
        self.physical_slots
            .iter()
            .filter(|slot| slot.active)
            .count()
    }

    /// Returns the total number of contacts,
    /// physical + virtual.
    pub fn contact_count(&self) -> usize {
        self.physical_contact_count() + usize::from(self.virtual_down)
    }

    /// Whether a virtual touch is active.
    pub fn virtual_touch_active(&self) -> bool {
        self.virtual_down
    }

    /// Current logical position of the virtual touch.
    pub fn virtual_position(&self) -> Point {
        self.virtual_position
    }

    /// evdev path of the virtual touchscreen.
    pub fn output_evdev_path(&self) -> Option<std::path::PathBuf> {
        self.output.evdev_path()
    }

    /// Processes currently available physical events.
    ///
    /// Returns the number of events consumed.
    pub fn poll(&mut self) -> io::Result<u64> {
        let events = match self.device.fetch_events() {
            Ok(events) => {
                /*
                 * The fetch_events() iterator holds a mutable
                 * borrow of self.device.
                 *
                 * We materialize the events to release that borrow
                 * before calling handle_event(), which also needs
                 * to mutate self.
                 */
                events.collect::<Vec<_>>()
            }

            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                return Ok(0);
            }

            Err(error) => {
                return Err(error);
            }
        };

        let count = events.len() as u64;

        for event in events {
            self.handle_event(event.event_type(), event.code(), event.value())?;
        }

        Ok(count)
    }

    /// Presses the virtual touch at the given coordinate.
    pub fn touch_down(&mut self, point: Point) -> io::Result<()> {
        if self.virtual_down {
            return Ok(());
        }

        let tracking_id = self.choose_virtual_tracking_id();

        self.virtual_position = self.clamp_point(point);

        self.virtual_down = true;

        self.virtual_tracking_id = Some(tracking_id);

        self.last_event_time = Some(Instant::now());

        self.emit_full_frame()
    }

    /// Moves the virtual touch.
    ///
    /// If the virtual touch is not active, only updates
    /// the position that will be used on the next `touch_down`.
    pub fn touch_move(&mut self, point: Point) -> io::Result<()> {
        self.virtual_position = self.clamp_point(point);

        self.last_event_time = Some(Instant::now());

        if self.virtual_down {
            self.emit_full_frame()?;
        }

        Ok(())
    }

    /// Releases the virtual touch.
    pub fn touch_up(&mut self) -> io::Result<()> {
        if !self.virtual_down {
            return Ok(());
        }

        self.virtual_down = false;

        self.virtual_tracking_id = None;

        self.last_event_time = Some(Instant::now());

        self.emit_full_frame()
    }

    /// Executes a complete tap.
    ///
    /// The duration is up to the caller; this function
    /// only does DOWN followed by UP.
    pub fn tap(&mut self, point: Point) -> io::Result<()> {
        self.touch_down(point)?;
        self.touch_up()
    }

    /// Sends a frame of the current state.
    ///
    /// Normally there is no need to call this manually:
    /// `poll`, `touch_down`, `touch_move` and `touch_up` already
    /// generate the necessary frames.
    pub fn sync(&mut self) -> io::Result<()> {
        self.emit_full_frame()
    }

    /// Approximate time of the last processed event.
    pub fn last_event_time(&self) -> Option<Instant> {
        self.last_event_time
    }

    fn handle_event(&mut self, event_type: EventType, code: u16, value: i32) -> io::Result<()> {
        if event_type == EventType::ABSOLUTE {
            self.handle_abs(code, value)?;

            return Ok(());
        }

        if event_type == EventType::SYNCHRONIZATION {
            if code == SYN_REPORT_CODE {
                self.emit_full_frame()?;
            }

            return Ok(());
        }

        /*
         * BTN_TOUCH and BTN_TOOL_FINGER from the physical device
         * are not copied. They are reconstructed from the combined
         * slot state.
         *
         * Other EV_KEY from the physical device are also ignored.
         */
        Ok(())
    }

    fn handle_abs(&mut self, code: u16, value: i32) -> io::Result<()> {
        match code {
            ABS_MT_SLOT_CODE => {
                let slot_count = self.physical_slots.len();

                if (0..slot_count as i32).contains(&value) {
                    self.current_physical_slot = value as usize;
                }
            }

            ABS_MT_TRACKING_ID_CODE => {
                let slot = &mut self.physical_slots[self.current_physical_slot];

                if value < 0 {
                    slot.active = false;

                    slot.tracking_id = None;
                } else {
                    slot.active = true;

                    slot.tracking_id = Some(value);
                }
            }

            ABS_MT_POSITION_X_CODE => {
                self.physical_slots[self.current_physical_slot].x = value;
            }

            ABS_MT_POSITION_Y_CODE => {
                self.physical_slots[self.current_physical_slot].y = value;
            }

            ABS_MT_TOUCH_MAJOR_CODE => {
                self.physical_slots[self.current_physical_slot].touch_major = value;
            }

            ABS_MT_WIDTH_MAJOR_CODE => {
                self.physical_slots[self.current_physical_slot].width_major = value;
            }

            _ => {}
        }

        Ok(())
    }

    fn emit_full_frame(&mut self) -> io::Result<()> {
        /*
         * Any active contact keeps BTN_TOUCH pressed.
         */
        self.output
            .set_touch_state(self.contact_count() > 0)
            .map_err(io::Error::other)?;

        /*
         * Reproduce all physical slots.
         */
        for index in 0..self.physical_slots.len() {
            let slot = self.physical_slots[index];

            let output_slot = OutputSlot {
                active: slot.active,

                tracking_id: slot.tracking_id,

                x: slot.x,
                y: slot.y,

                touch_major: slot.touch_major,

                width_major: slot.width_major,
            };

            self.output
                .set_slot(index, output_slot)
                .map_err(io::Error::other)?;
        }

        /*
         * Virtual slot.
         */
        let virtual_slot = self.virtual_slot();

        let virtual_state = self.virtual_slot_state();

        self.output
            .set_slot(
                virtual_slot,
                OutputSlot {
                    active: virtual_state.active,

                    tracking_id: virtual_state.tracking_id,

                    x: virtual_state.x,

                    y: virtual_state.y,

                    touch_major: virtual_state.touch_major,

                    width_major: virtual_state.width_major,
                },
            )
            .map_err(io::Error::other)?;

        /*
         * Single SYN_REPORT for the complete frame.
         */
        self.output.sync().map_err(io::Error::other)?;

        Ok(())
    }

    fn virtual_slot_state(&self) -> SlotState {
        let x_range = self.touchscreen.x_range();

        let y_range = self.touchscreen.y_range();

        SlotState {
            active: self.virtual_down,

            tracking_id: self.virtual_tracking_id,

            x: display_to_raw(
                self.virtual_position.x,
                0,
                self.display_size.width - 1,
                x_range.min,
                x_range.max,
            ),

            y: display_to_raw(
                self.virtual_position.y,
                0,
                self.display_size.height - 1,
                y_range.min,
                y_range.max,
            ),

            touch_major: self.virtual_touch_major,

            width_major: self.virtual_width_major,
        }
    }

    fn clamp_point(&self, point: Point) -> Point {
        Point {
            x: point.x.clamp(0, self.display_size.width - 1),

            y: point.y.clamp(0, self.display_size.height - 1),
        }
    }

    fn choose_virtual_tracking_id(&self) -> i32 {
        let range = self.touchscreen.tracking_id_range();

        let start = range.max.saturating_sub(1024).max(range.min);

        for candidate in start..range.max {
            let in_use = self
                .physical_slots
                .iter()
                .any(|slot| slot.active && slot.tracking_id == Some(candidate));

            if !in_use {
                return candidate;
            }
        }

        /*
         * Fallback.
         */
        range.min
    }
}

impl Drop for TouchMultiplexer {
    fn drop(&mut self) {
        /*
         * Returns the physical device to Android.
         */
        let _ = self.device.ungrab();

        /*
         * VirtualTouchscreen is automatically dropped afterwards,
         * removing the uinput device.
         */
    }
}

fn display_to_raw(
    value: i32,
    display_min: i32,
    display_max: i32,
    raw_min: i32,
    raw_max: i32,
) -> i32 {
    let value = value.clamp(display_min, display_max);

    if display_max == display_min {
        return raw_min;
    }

    raw_min
        + (((value - display_min) as i64 * (raw_max - raw_min) as i64)
            / (display_max - display_min) as i64) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_to_raw_identity() {
        assert_eq!(display_to_raw(500, 0, 1000, 0, 1000), 500);
    }

    #[test]
    fn display_to_raw_scaled() {
        assert_eq!(display_to_raw(0, 0, 719, 0, 1079), 0);
        assert_eq!(display_to_raw(719, 0, 719, 0, 1079), 1079);
        assert_eq!(display_to_raw(360, 0, 719, 0, 1079), 540);
    }

    #[test]
    fn display_to_raw_different_offsets() {
        assert_eq!(display_to_raw(5, 0, 10, 100, 200), 150);
        assert_eq!(display_to_raw(0, 0, 10, 100, 200), 100);
        assert_eq!(display_to_raw(10, 0, 10, 100, 200), 200);
    }

    #[test]
    fn display_to_raw_clamps_value() {
        assert_eq!(display_to_raw(-5, 0, 100, 0, 1000), 0);
        assert_eq!(display_to_raw(200, 0, 100, 0, 1000), 1000);
    }

    #[test]
    fn display_to_raw_equal_display_range() {
        assert_eq!(display_to_raw(5, 10, 10, 0, 1000), 0);
    }

    #[test]
    fn display_to_raw_reversed_ranges() {
        assert_eq!(display_to_raw(0, 0, 100, 1000, 0), 1000);
        assert_eq!(display_to_raw(100, 0, 100, 1000, 0), 0);
        assert_eq!(display_to_raw(50, 0, 100, 1000, 0), 500);
    }

    #[test]
    fn display_size_default() {
        let ds = DisplaySize::default();
        assert_eq!(ds.width, 720);
        assert_eq!(ds.height, 1600);
    }

    #[test]
    fn display_size_new() {
        let ds = DisplaySize::new(1080, 2400);
        assert_eq!(ds.width, 1080);
        assert_eq!(ds.height, 2400);
    }

    #[test]
    fn point_new() {
        let p = Point::new(100, 200);
        assert_eq!(p.x, 100);
        assert_eq!(p.y, 200);
    }

    #[test]
    fn point_copy() {
        let p1 = Point::new(10, 20);
        let p2 = p1;
        assert_eq!(p1, p2);
    }

    #[test]
    fn display_size_copy() {
        let d1 = DisplaySize::new(100, 200);
        let d2 = d1;
        assert_eq!(d1, d2);
    }
}
