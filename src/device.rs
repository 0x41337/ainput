use std::fmt;
use std::path::{Path, PathBuf};

use evdev::{AbsoluteAxisCode, Device, EventType, KeyCode, PropType};

/// Describes the range of an absolute axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AxisRange {
    pub min: i32,
    pub max: i32,
}

impl AxisRange {
    pub fn new(min: i32, max: i32) -> Self {
        Self { min, max }
    }

    pub fn len(&self) -> i32 {
        self.max - self.min + 1
    }
}

/// Optional range information for a contact dimension.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OptionalAxisRange {
    pub min: i32,
    pub max: i32,
}

/// Errors specific to autodetection.
#[derive(Debug)]
pub enum TouchDeviceError {
    Io(std::io::Error),

    NoTouchscreen,

    InvalidDevice { path: PathBuf, reason: String },

    Ambiguous { candidates: Vec<TouchDevice> },
}

impl fmt::Display for TouchDeviceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => {
                write!(f, "{error}")
            }

            Self::NoTouchscreen => {
                write!(f, "no compatible direct MT touchscreen found")
            }

            Self::InvalidDevice { path, reason } => {
                write!(
                    f,
                    "{} is not a compatible touchscreen: {}",
                    path.display(),
                    reason
                )
            }

            Self::Ambiguous { candidates } => {
                writeln!(
                    f,
                    "multiple touchscreen candidates with the same priority:"
                )?;

                for candidate in candidates {
                    writeln!(f, "  {}", candidate.summary())?;
                }

                write!(
                    f,
                    "set AINPUT_TOUCH_DEVICE to explicitly choose one"
                )
            }
        }
    }
}

impl std::error::Error for TouchDeviceError {}

impl From<std::io::Error> for TouchDeviceError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// Description of a physical touchscreen detected by evdev.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TouchDevice {
    path: PathBuf,
    name: String,

    slot_range: AxisRange,
    x_range: AxisRange,
    y_range: AxisRange,

    tracking_id_range: AxisRange,

    touch_major_range: Option<OptionalAxisRange>,
    width_major_range: Option<OptionalAxisRange>,

    has_btn_touch: bool,
    has_btn_tool_finger: bool,
    direct: bool,

    score: i32,
}

impl TouchDevice {
    /// Autodetects the best available touchscreen.
    ///
    /// You can explicitly choose a device via:
    ///
    /// AINPUT_TOUCH_DEVICE=/dev/input/event7
    pub fn detect() -> Result<Self, TouchDeviceError> {
        if let Ok(path) = std::env::var("AINPUT_TOUCH_DEVICE") {
            return Self::from_path(path);
        }

        Self::detect_automatic()
    }

    /// Opens and explicitly inspects an evdev path.
    pub fn from_path<P>(path: P) -> Result<Self, TouchDeviceError>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref().to_path_buf();

        let device = Device::open(&path)?;

        match Self::inspect(path.clone(), &device)? {
            Some(device) => Ok(device),

            None => Err(TouchDeviceError::InvalidDevice {
                path,
                reason: "device does not have the required direct MT touchscreen signature"
                    .to_string(),
            }),
        }
    }

    fn detect_automatic() -> Result<Self, TouchDeviceError> {
        let mut candidates = Vec::<TouchDevice>::new();

        for (path, device) in evdev::enumerate() {
            match Self::inspect(path, &device) {
                Ok(Some(candidate)) => {
                    candidates.push(candidate);
                }

                Ok(None) => {}

                Err(error) => {
                    eprintln!("[ainput] ignoring device during autodetection: {}", error);
                }
            }
        }

        if candidates.is_empty() {
            return Err(TouchDeviceError::NoTouchscreen);
        }

        /*
         * Highest score first.
         */
        candidates.sort_by_key(|candidate| std::cmp::Reverse(candidate.score));

        let best_score = candidates[0].score;

        let best_count = candidates
            .iter()
            .filter(|candidate| candidate.score == best_score)
            .count();

        /*
         * Do not silently choose when there is a tie.
         */
        if best_count > 1 {
            let tied = candidates
                .into_iter()
                .filter(|candidate| candidate.score == best_score)
                .collect();

            return Err(TouchDeviceError::Ambiguous { candidates: tied });
        }

        Ok(candidates.remove(0))
    }

    fn inspect(path: PathBuf, device: &Device) -> Result<Option<Self>, TouchDeviceError> {
        let name = device.name().unwrap_or("<unnamed>").to_string();

        /*
         * Must support EV_ABS.
         */
        if !device.supported_events().contains(EventType::ABSOLUTE) {
            return Ok(None);
        }

        /*
         * Minimum multitouch B protocol signature.
         */
        let required_axes = [
            AbsoluteAxisCode::ABS_MT_SLOT,
            AbsoluteAxisCode::ABS_MT_TRACKING_ID,
            AbsoluteAxisCode::ABS_MT_POSITION_X,
            AbsoluteAxisCode::ABS_MT_POSITION_Y,
        ];

        let supports_required_axes = required_axes
            .iter()
            .all(|axis| Self::has_abs(device, *axis));

        if !supports_required_axes {
            return Ok(None);
        }

        /*
         * BTN_TOUCH.
         */
        let has_btn_touch = Self::has_key(device, KeyCode::BTN_TOUCH);

        if !has_btn_touch {
            return Ok(None);
        }

        /*
         * INPUT_PROP_DIRECT.
         *
         * This excludes touchpads and other indirect devices.
         */
        let direct = device.properties().contains(PropType::DIRECT);

        if !direct {
            return Ok(None);
        }

        /*
         * Required AbsInfo.
         */
        let slot = Self::abs_info(device, AbsoluteAxisCode::ABS_MT_SLOT)?.ok_or_else(|| {
            TouchDeviceError::InvalidDevice {
                path: path.clone(),
                    reason: "ABS_MT_SLOT advertised without AbsInfo".to_string(),
            }
        })?;

        let tracking =
            Self::abs_info(device, AbsoluteAxisCode::ABS_MT_TRACKING_ID)?.ok_or_else(|| {
                TouchDeviceError::InvalidDevice {
                    path: path.clone(),
                    reason: "ABS_MT_TRACKING_ID advertised without AbsInfo".to_string(),
                }
            })?;

        let x = Self::abs_info(device, AbsoluteAxisCode::ABS_MT_POSITION_X)?.ok_or_else(|| {
            TouchDeviceError::InvalidDevice {
                path: path.clone(),
                    reason: "ABS_MT_POSITION_X advertised without AbsInfo".to_string(),
            }
        })?;

        let y = Self::abs_info(device, AbsoluteAxisCode::ABS_MT_POSITION_Y)?.ok_or_else(|| {
            TouchDeviceError::InvalidDevice {
                path: path.clone(),
                    reason: "ABS_MT_POSITION_Y advertised without AbsInfo".to_string(),
            }
        })?;

        /*
         * Range validation.
         */
        if slot.minimum() != 0 || slot.maximum() < slot.minimum() {
            return Ok(None);
        }

        if x.maximum() <= x.minimum() || y.maximum() <= y.minimum() {
            return Ok(None);
        }

        if tracking.maximum() < tracking.minimum() {
            return Ok(None);
        }

        let slot_count = slot.maximum() - slot.minimum() + 1;

        /*
         * Defensive limit.
         */
        if !(1..=64).contains(&slot_count) {
            return Ok(None);
        }

        /*
         * Optional information.
         */
        let touch_major = Self::abs_info(device, AbsoluteAxisCode::ABS_MT_TOUCH_MAJOR)?;

        let width_major = Self::abs_info(device, AbsoluteAxisCode::ABS_MT_WIDTH_MAJOR)?;

        let has_btn_tool_finger = Self::has_key(device, KeyCode::BTN_TOOL_FINGER);

        /*
         * Score.
         *
         * All fundamental fields are already required.
         */
        let mut score = 100;

        if has_btn_tool_finger {
            score += 10;
        }

        if touch_major.is_some() {
            score += 5;
        }

        if width_major.is_some() {
            score += 5;
        }

        if slot_count >= 2 {
            score += 10;
        }

        /*
         * Penalty for devices that look virtual.
         */
        if Self::looks_virtual(&name, &path) {
            score -= 1000;
        }

        Ok(Some(Self {
            path,
            name,

            slot_range: AxisRange::new(slot.minimum(), slot.maximum()),

            x_range: AxisRange::new(x.minimum(), x.maximum()),

            y_range: AxisRange::new(y.minimum(), y.maximum()),

            tracking_id_range: AxisRange::new(tracking.minimum(), tracking.maximum()),

            touch_major_range: touch_major.map(|info| OptionalAxisRange {
                min: info.minimum(),
                max: info.maximum(),
            }),

            width_major_range: width_major.map(|info| OptionalAxisRange {
                min: info.minimum(),
                max: info.maximum(),
            }),

            has_btn_touch,
            has_btn_tool_finger,
            direct,

            score,
        }))
    }

    fn abs_info(
        device: &Device,
        axis: AbsoluteAxisCode,
    ) -> Result<Option<evdev::AbsInfo>, TouchDeviceError> {
        let infos = device
            .get_absinfo()
            .map_err(|error| TouchDeviceError::Io(error))?;

        Ok(infos
            .into_iter()
            .find(|(candidate, _)| *candidate == axis)
            .map(|(_, info)| info))
    }

    fn has_abs(device: &Device, axis: AbsoluteAxisCode) -> bool {
        device
            .supported_absolute_axes()
            .map(|axes| axes.contains(axis))
            .unwrap_or(false)
    }

    fn has_key(device: &Device, key: KeyCode) -> bool {
        device
            .supported_keys()
            .map(|keys| keys.contains(key))
            .unwrap_or(false)
    }

    fn looks_virtual(name: &str, path: &Path) -> bool {
        let name = name.to_ascii_lowercase();

        let path = path.to_string_lossy().to_ascii_lowercase();

        const TOKENS: &[&str] = &["uinput", "virtual", "injector", "virtio"];

        TOKENS
            .iter()
            .any(|token| name.contains(token) || path.contains(token))
    }

    /// evdev device path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Name provided by the kernel.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Raw slot range.
    pub fn slot_range(&self) -> AxisRange {
        self.slot_range
    }

    /// Number of slots.
    pub fn slot_count(&self) -> usize {
        self.slot_range.len() as usize
    }

    /// Raw X range.
    pub fn x_range(&self) -> AxisRange {
        self.x_range
    }

    /// Raw Y range.
    pub fn y_range(&self) -> AxisRange {
        self.y_range
    }

    /// Tracking ID range.
    pub fn tracking_id_range(&self) -> AxisRange {
        self.tracking_id_range
    }

    /// TOUCH_MAJOR range, if supported.
    pub fn touch_major_range(&self) -> Option<OptionalAxisRange> {
        self.touch_major_range
    }

    /// WIDTH_MAJOR range, if supported.
    pub fn width_major_range(&self) -> Option<OptionalAxisRange> {
        self.width_major_range
    }

    /// Whether BTN_TOUCH is present.
    pub fn has_btn_touch(&self) -> bool {
        self.has_btn_touch
    }

    /// Whether BTN_TOOL_FINGER is present.
    pub fn has_btn_tool_finger(&self) -> bool {
        self.has_btn_tool_finger
    }

    /// Whether INPUT_PROP_DIRECT is present.
    pub fn is_direct(&self) -> bool {
        self.direct
    }

    /// Score used internally by the detector.
    pub fn score(&self) -> i32 {
        self.score
    }

    /// Useful summary for logs/debug.
    pub fn summary(&self) -> String {
        format!(
            "{} [{}] slots={} X={}..{} Y={}..{} tracking={}..{} direct={} score={}",
            self.path.display(),
            self.name,
            self.slot_count(),
            self.x_range.min,
            self.x_range.max,
            self.y_range.min,
            self.y_range.max,
            self.tracking_id_range.min,
            self.tracking_id_range.max,
            self.direct,
            self.score,
        )
    }

    /// Reopens the discovered device.
    ///
    /// We keep opening separate from the description so the
    /// library can perform subsequent evdev configuration.
    pub fn open(&self) -> Result<Device, TouchDeviceError> {
        Device::open(&self.path).map_err(TouchDeviceError::Io)
    }
}
