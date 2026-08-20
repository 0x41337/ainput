use std::fs::OpenOptions;
use std::io;
use std::os::fd::IntoRawFd;

use input_linux::{
    AbsoluteAxis, AbsoluteEvent, AbsoluteInfo, AbsoluteInfoSetup, EventKind, EventTime, InputId,
    InputProperty, Key, KeyEvent as LinuxKeyEvent, KeyState, SynchronizeEvent, SynchronizeKind,
    uinput::UInputHandle,
};

use crate::device::TouchDevice;

/// State of a slot that can be sent to the virtual touchscreen.
///
/// This struct does not contain multiplexing logic. It only
/// describes the state that should be written to the virtual device.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OutputSlot {
    pub active: bool,
    pub tracking_id: Option<i32>,
    pub x: i32,
    pub y: i32,
    pub touch_major: i32,
    pub width_major: i32,
}

/// Error related to the `uinput` device.
#[derive(Debug)]
pub enum UInputError {
    Io(io::Error),

    InvalidSlot { slot: usize, slot_count: usize },

    ActiveSlotWithoutTrackingId { slot: usize },
}

impl std::fmt::Display for UInputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => {
                write!(f, "{error}")
            }

            Self::InvalidSlot { slot, slot_count } => {
                write!(
                    f,
                    "slot {} is invalid; device has {} slots",
                    slot, slot_count
                )
            }

            Self::ActiveSlotWithoutTrackingId { slot } => {
                write!(f, "slot {} is active without tracking_id", slot)
            }
        }
    }
}

impl std::error::Error for UInputError {}

impl From<io::Error> for UInputError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Virtual touchscreen device created via `/dev/uinput`.
///
/// O dispositivo usa:
///
/// - `EV_ABS`
/// - `ABS_MT_SLOT`
/// - `ABS_MT_TRACKING_ID`
/// - `ABS_MT_POSITION_X`
/// - `ABS_MT_POSITION_Y`
/// - `ABS_MT_TOUCH_MAJOR`, quando disponível
/// - `ABS_MT_WIDTH_MAJOR`, quando disponível
/// - `BTN_TOUCH`
/// - `BTN_TOOL_FINGER`
/// - `INPUT_PROP_DIRECT`
pub struct VirtualTouchscreen {
    handle: UInputHandle<i32>,

    physical_slot_count: usize,
    total_slot_count: usize,

    x_min: i32,
    x_max: i32,

    y_min: i32,
    y_max: i32,

    tracking_id_min: i32,
    tracking_id_max: i32,

    touch_major_range: Option<(i32, i32)>,
    width_major_range: Option<(i32, i32)>,

    output_slots: Vec<OutputSlot>,

    btn_touch: bool,
    btn_tool_finger: bool,
}

impl VirtualTouchscreen {
    /// Creates the virtual touchscreen using the characteristics
    /// discovered on the physical touchscreen.
    ///
    /// An additional slot is created beyond the physical slots.
    ///
    /// Example:
    ///
    /// `10 physical slots -> virtual slot = 10`
    pub fn create(device: &TouchDevice) -> Result<Self, UInputError> {
        let physical_slot_count = device.slot_count();

        if physical_slot_count == 0 {
            return Err(UInputError::Io(io::Error::other("touchscreen without slots")));
        }

        /*
         * One additional slot for the virtual contact.
         */
        let total_slot_count = physical_slot_count + 1;

        /*
         * ----------------------------------------------------
         * Abre /dev/uinput.
         * ----------------------------------------------------
         */
        let file = OpenOptions::new().write(true).open("/dev/uinput")?;

        let fd = file.into_raw_fd();

        let handle = UInputHandle::new(fd);

        /*
         * ----------------------------------------------------
         * EV_ABS
         * ----------------------------------------------------
         */
        handle
            .set_evbit(EventKind::Absolute)
            .map_err(io::Error::other)?;

        /*
         * ----------------------------------------------------
         * EV_KEY
         * ----------------------------------------------------
         */
        handle.set_evbit(EventKind::Key).map_err(io::Error::other)?;

        /*
         * ----------------------------------------------------
         * INPUT_PROP_DIRECT
         * ----------------------------------------------------
         */
        handle
            .set_propbit(InputProperty::Direct)
            .map_err(io::Error::other)?;

        /*
         * ----------------------------------------------------
         * BTN_TOUCH
         * ----------------------------------------------------
         */
        handle
            .set_keybit(Key::ButtonTouch)
            .map_err(io::Error::other)?;

        /*
         * ----------------------------------------------------
         * BTN_TOOL_FINGER
         * ----------------------------------------------------
         */
        handle
            .set_keybit(Key::ButtonToolFinger)
            .map_err(io::Error::other)?;

        /*
         * ----------------------------------------------------
         * ABS_MT_SLOT
         * ----------------------------------------------------
         */
        handle
            .set_absbit(AbsoluteAxis::MultitouchSlot)
            .map_err(io::Error::other)?;

        /*
         * ----------------------------------------------------
         * ABS_MT_TRACKING_ID
         * ----------------------------------------------------
         */
        handle
            .set_absbit(AbsoluteAxis::MultitouchTrackingId)
            .map_err(io::Error::other)?;

        /*
         * ----------------------------------------------------
         * ABS_MT_POSITION_X
         * ----------------------------------------------------
         */
        handle
            .set_absbit(AbsoluteAxis::MultitouchPositionX)
            .map_err(io::Error::other)?;

        /*
         * ----------------------------------------------------
         * ABS_MT_POSITION_Y
         * ----------------------------------------------------
         */
        handle
            .set_absbit(AbsoluteAxis::MultitouchPositionY)
            .map_err(io::Error::other)?;

        /*
         * ----------------------------------------------------
         * TOUCH_MAJOR
         * ----------------------------------------------------
         */
        if device.touch_major_range().is_some() {
            handle
                .set_absbit(AbsoluteAxis::MultitouchTouchMajor)
                .map_err(io::Error::other)?;
        }

        /*
         * ----------------------------------------------------
         * WIDTH_MAJOR
         * ----------------------------------------------------
         */
        if device.width_major_range().is_some() {
            handle
                .set_absbit(AbsoluteAxis::MultitouchWidthMajor)
                .map_err(io::Error::other)?;
        }

        let slot_range = device.slot_range();

        let tracking_range = device.tracking_id_range();

        let x_range = device.x_range();

        let y_range = device.y_range();

        /*
         * ----------------------------------------------------
         * ABS_MT_SLOT
         *
         * The last slot is the virtual one.
         * ----------------------------------------------------
         */
        let slot_info = AbsoluteInfoSetup {
            axis: AbsoluteAxis::MultitouchSlot,

            info: AbsoluteInfo {
                value: slot_range.min,

                minimum: slot_range.min,

                maximum: physical_slot_count as i32,

                fuzz: 0,
                flat: 0,
                resolution: 0,
            },
        };

        /*
         * ----------------------------------------------------
         * ABS_MT_TRACKING_ID
         * ----------------------------------------------------
         */
        let tracking_info = AbsoluteInfoSetup {
            axis: AbsoluteAxis::MultitouchTrackingId,

            info: AbsoluteInfo {
                value: -1,

                minimum: tracking_range.min,

                maximum: tracking_range.max,

                fuzz: 0,
                flat: 0,
                resolution: 0,
            },
        };

        /*
         * ----------------------------------------------------
         * ABS_MT_POSITION_X
         * ----------------------------------------------------
         */
        let x_info = AbsoluteInfoSetup {
            axis: AbsoluteAxis::MultitouchPositionX,

            info: AbsoluteInfo {
                value: x_range.min,

                minimum: x_range.min,

                maximum: x_range.max,

                fuzz: 0,
                flat: 0,
                resolution: 0,
            },
        };

        /*
         * ----------------------------------------------------
         * ABS_MT_POSITION_Y
         * ----------------------------------------------------
         */
        let y_info = AbsoluteInfoSetup {
            axis: AbsoluteAxis::MultitouchPositionY,

            info: AbsoluteInfo {
                value: y_range.min,

                minimum: y_range.min,

                maximum: y_range.max,

                fuzz: 0,
                flat: 0,
                resolution: 0,
            },
        };

        let mut abs_info = vec![slot_info, tracking_info, x_info, y_info];

        /*
         * ----------------------------------------------------
         * TOUCH_MAJOR
         * ----------------------------------------------------
         */
        if let Some(range) = device.touch_major_range() {
            abs_info.push(AbsoluteInfoSetup {
                axis: AbsoluteAxis::MultitouchTouchMajor,

                info: AbsoluteInfo {
                    value: range.min,

                    minimum: range.min,

                    maximum: range.max,

                    fuzz: 0,
                    flat: 0,
                    resolution: 0,
                },
            });
        }

        /*
         * ----------------------------------------------------
         * WIDTH_MAJOR
         * ----------------------------------------------------
         */
        if let Some(range) = device.width_major_range() {
            abs_info.push(AbsoluteInfoSetup {
                axis: AbsoluteAxis::MultitouchWidthMajor,

                info: AbsoluteInfo {
                    value: range.min,

                    minimum: range.min,

                    maximum: range.max,

                    fuzz: 0,
                    flat: 0,
                    resolution: 0,
                },
            });
        }

        /*
         * ----------------------------------------------------
         * InputId
         * ----------------------------------------------------
         */
        let id = InputId {
            bustype: 0x06,
            vendor: 0,
            product: 0,
            version: 1,
        };

        /*
         * ----------------------------------------------------
         * Criação do dispositivo.
         * ----------------------------------------------------
         */
        handle
            .create(&id, b"Input Injector", 0, &abs_info)
            .map_err(io::Error::other)?;

        Ok(Self {
            handle,

            physical_slot_count,

            total_slot_count,

            x_min: x_range.min,

            x_max: x_range.max,

            y_min: y_range.min,

            y_max: y_range.max,

            tracking_id_min: tracking_range.min,

            tracking_id_max: tracking_range.max,

            touch_major_range: device
                .touch_major_range()
                .map(|range| (range.min, range.max)),

            width_major_range: device
                .width_major_range()
                .map(|range| (range.min, range.max)),

            output_slots: vec![OutputSlot::default(); total_slot_count],

            btn_touch: false,
            btn_tool_finger: false,
        })
    }

    /// Number of physical slots.
    pub fn physical_slot_count(&self) -> usize {
        self.physical_slot_count
    }

    /// Total number of slots available on the output.
    pub fn total_slot_count(&self) -> usize {
        self.total_slot_count
    }

    /// Index of the slot reserved for the virtual contact.
    pub fn virtual_slot(&self) -> usize {
        self.physical_slot_count
    }

    /// X range of the device.
    pub fn x_range(&self) -> (i32, i32) {
        (self.x_min, self.x_max)
    }

    /// Y range of the device.
    pub fn y_range(&self) -> (i32, i32) {
        (self.y_min, self.y_max)
    }

    /// Tracking ID range.
    pub fn tracking_id_range(&self) -> (i32, i32) {
        (self.tracking_id_min, self.tracking_id_max)
    }

    /// Validates the slot index.
    fn validate_slot(&self, slot: usize) -> Result<(), UInputError> {
        if slot >= self.total_slot_count {
            return Err(UInputError::InvalidSlot {
                slot,
                slot_count: self.total_slot_count,
            });
        }

        Ok(())
    }

    /// Writes the state of a slot to the virtual device.
    ///
    /// Does not send `SYN_REPORT`.
    ///
    /// The caller can update multiple slots and call `sync()`
    /// only at the end of the frame.
    pub fn set_slot(&mut self, slot: usize, state: OutputSlot) -> Result<(), UInputError> {
        self.validate_slot(slot)?;

        if !state.active {
            self.select_slot(slot)?;

            self.send_tracking_id(-1)?;

            self.output_slots[slot] = OutputSlot::default();

            return Ok(());
        }

        let tracking_id = state
            .tracking_id
            .ok_or(UInputError::ActiveSlotWithoutTrackingId { slot })?;

        self.select_slot(slot)?;

        self.send_tracking_id(tracking_id)?;

        self.send_position_x(state.x)?;

        self.send_position_y(state.y)?;

        if self.touch_major_range.is_some() {
            self.send_touch_major(state.touch_major)?;
        }

        if self.width_major_range.is_some() {
            self.send_width_major(state.width_major)?;
        }

        self.output_slots[slot] = state;

        Ok(())
    }

    /// Releases a slot.
    ///
    /// Does not send `SYN_REPORT`.
    pub fn release_slot(&mut self, slot: usize) -> Result<(), UInputError> {
        self.validate_slot(slot)?;

        self.select_slot(slot)?;

        self.send_tracking_id(-1)?;

        self.output_slots[slot] = OutputSlot::default();

        Ok(())
    }

    /// Returns the state currently held in the output.
    pub fn output_slot(&self, slot: usize) -> Result<OutputSlot, UInputError> {
        self.validate_slot(slot)?;

        Ok(self.output_slots[slot])
    }

    /// Updates `BTN_TOUCH` and `BTN_TOOL_FINGER`.
    ///
    /// The global state is the multiplexer's responsibility;
    /// this function only writes the requested state.
    pub fn set_touch_state(&mut self, pressed: bool) -> Result<(), UInputError> {
        if pressed == self.btn_touch {
            return Ok(());
        }

        self.send_key(Key::ButtonToolFinger, pressed)?;

        self.send_key(Key::ButtonTouch, pressed)?;

        self.btn_touch = pressed;

        self.btn_tool_finger = pressed;

        Ok(())
    }

    /// Whether BTN_TOUCH is pressed.
    pub fn touch_pressed(&self) -> bool {
        self.btn_touch
    }

    /// Sends `SYN_REPORT`.
    pub fn sync(&mut self) -> Result<(), UInputError> {
        self.send_sync()?;

        Ok(())
    }

    /// evdev path of the virtual device, when available.
    pub fn evdev_path(&self) -> Option<std::path::PathBuf> {
        self.handle.evdev_path().ok()
    }

    fn select_slot(&mut self, slot: usize) -> Result<(), UInputError> {
        self.send_absolute(AbsoluteAxis::MultitouchSlot, slot as i32)
    }

    fn send_tracking_id(&mut self, value: i32) -> Result<(), UInputError> {
        self.send_absolute(AbsoluteAxis::MultitouchTrackingId, value)
    }

    fn send_position_x(&mut self, value: i32) -> Result<(), UInputError> {
        let value = value.clamp(self.x_min, self.x_max);

        self.send_absolute(AbsoluteAxis::MultitouchPositionX, value)
    }

    fn send_position_y(&mut self, value: i32) -> Result<(), UInputError> {
        let value = value.clamp(self.y_min, self.y_max);

        self.send_absolute(AbsoluteAxis::MultitouchPositionY, value)
    }

    fn send_touch_major(&mut self, value: i32) -> Result<(), UInputError> {
        let Some((minimum, maximum)) = self.touch_major_range else {
            return Ok(());
        };

        let value = value.clamp(minimum, maximum);

        self.send_absolute(AbsoluteAxis::MultitouchTouchMajor, value)
    }

    fn send_width_major(&mut self, value: i32) -> Result<(), UInputError> {
        let Some((minimum, maximum)) = self.width_major_range else {
            return Ok(());
        };

        let value = value.clamp(minimum, maximum);

        self.send_absolute(AbsoluteAxis::MultitouchWidthMajor, value)
    }

    fn send_absolute(&mut self, axis: AbsoluteAxis, value: i32) -> Result<(), UInputError> {
        let event = AbsoluteEvent::new(EventTime::new(0, 0), axis, value).into_event();

        self.write_event(&event)
    }

    fn send_key(&mut self, key: Key, pressed: bool) -> Result<(), UInputError> {
        let event =
            LinuxKeyEvent::new(EventTime::new(0, 0), key, KeyState::pressed(pressed)).into_event();

        self.write_event(&event)
    }

    fn send_sync(&mut self) -> Result<(), UInputError> {
        let event =
            SynchronizeEvent::new(EventTime::new(0, 0), SynchronizeKind::Report, 0).into_event();

        self.write_event(&event)
    }

    fn write_event(&mut self, event: &input_linux::InputEvent) -> Result<(), UInputError> {
        let raw_event: &input_linux::sys::input_event = event.into();

        self.handle
            .write(std::slice::from_ref(raw_event))
            .map(|_| ())
            .map_err(io::Error::other)
            .map_err(UInputError::Io)
    }
}

impl Drop for VirtualTouchscreen {
    fn drop(&mut self) {
        /*
         * The Drop ensures the virtual device is removed
         * even if the caller exits with an error.
         */
        let _ = self.handle.dev_destroy();
    }
}
