use ratatui::style::Color;
use strum::{AsRefStr, Display, EnumString, IntoStaticStr, VariantNames};
use zbus::zvariant::OwnedObjectPath;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnitEditMode {
    Override,
    Full,
}

impl UnitEditMode {
    pub fn action_label(self) -> &'static str {
        match self {
            Self::Override => "override",
            Self::Full => "full replacement",
        }
    }

    pub fn draft_label(self, unit_name: &str) -> String {
        match self {
            Self::Override => format!("Draft Override: {unit_name}"),
            Self::Full => format!("Draft Replacement: {unit_name}"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct EditRequest {
    pub unit_name: String,
    pub scope: UnitScope,
    pub mode: UnitEditMode,
    pub initial_content: String,
    pub restore_content: String,
    pub restore_path: String,
}

#[derive(Clone, Debug)]
pub struct EditReview {
    pub unit_name: String,
    pub scope: UnitScope,
    pub mode: UnitEditMode,
    pub edited_content: String,
    pub restore_content: String,
    pub restore_path: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnitAction {
    Start,
    Stop,
    Restart,
    Reload,
    Enable,
    Disable,
    Mask,
    Unmask,
    ResetFailed,
}

impl UnitAction {
    pub fn past_tense(self) -> &'static str {
        match self {
            Self::Start => "Started",
            Self::Stop => "Stopped",
            Self::Restart => "Restarted",
            Self::Reload => "Reloaded",
            Self::Enable => "Enabled",
            Self::Disable => "Disabled",
            Self::Mask => "Masked",
            Self::Unmask => "Unmasked",
            Self::ResetFailed => "Reset failed state for",
        }
    }
}

#[derive(Clone, Debug)]
pub enum PrivilegedAction {
    UnitCommand {
        unit_name: String,
        scope: UnitScope,
        action: UnitAction,
    },
    ApplyEdit {
        unit_name: String,
        scope: UnitScope,
        mode: UnitEditMode,
        content: String,
    },
}

pub struct AttemptResult {
    pub success: bool,
    pub error: Option<String>,
}

pub enum AppInternalEvent {
    PtyOutput(String),
    PtyClosed,
    AuthResult(AttemptResult),
    UnitsLoaded(Vec<UnitInfo>, bool),
    LogsLoaded(Vec<String>, bool),
    LogLineReceived(String),
    FileLoaded(String, String),
    Error(String),
    ClearNotification,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NotificationType {
    Success,
    Error,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Notification {
    pub message: String,
    pub kind: NotificationType,
}

pub enum PendingAction {
    EditFile(EditRequest),
    EditText { filename: String, content: String },
}

#[derive(Clone, Debug)]
pub struct UnitInfo {
    pub name: String,
    pub description: String,
    pub scope: UnitScope,
    pub load_state: UnitLoadState,
    pub active_state: UnitActiveState,
    pub enablement_state: UnitEnablementState,
    pub sub_state: String,
    pub path: OwnedObjectPath,
    pub fragment_path: String,
}

#[derive(
    Debug,
    Clone,
    Copy,
    Eq,
    PartialEq,
    Hash,
    Ord,
    PartialOrd,
    Display,
    EnumString,
    AsRefStr,
    IntoStaticStr,
    VariantNames,
)]
#[strum(serialize_all = "kebab-case")]
pub enum UnitType {
    Unknown,
    Service,
    Socket,
    Target,
    Device,
    Mount,
    Automount,
    Timer,
    Path,
    Slice,
    Scope,
    Swap,
}

impl UnitType {
    pub fn from_unit_name(unit_name: &str) -> Self {
        unit_name
            .rsplit_once('.')
            .map_or(Self::Unknown, |(_, suffix)| {
                suffix.parse().unwrap_or(Self::Unknown)
            })
    }

    pub fn color(self) -> Color {
        match self {
            Self::Unknown => Color::Gray,
            Self::Service => Color::Green,
            Self::Socket => Color::Cyan,
            Self::Target => Color::Yellow,
            Self::Device => Color::Blue,
            Self::Mount => Color::Magenta,
            Self::Automount => Color::LightMagenta,
            Self::Timer => Color::Red,
            Self::Path => Color::White,
            Self::Slice => Color::LightCyan,
            Self::Scope => Color::Gray,
            Self::Swap => Color::LightRed,
        }
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    Eq,
    PartialEq,
    Hash,
    Ord,
    PartialOrd,
    Default,
    Display,
    EnumString,
    AsRefStr,
    IntoStaticStr,
    VariantNames,
)]
#[strum(serialize_all = "kebab-case")]
pub enum UnitScope {
    #[default]
    Global,
    Session,
}

impl UnitScope {
    pub fn color(self) -> Color {
        match self {
            Self::Global => Color::Blue,
            Self::Session => Color::Cyan,
        }
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    Eq,
    PartialEq,
    Hash,
    Ord,
    PartialOrd,
    Default,
    Display,
    EnumString,
    AsRefStr,
    IntoStaticStr,
    VariantNames,
)]
#[strum(serialize_all = "kebab-case")]
pub enum UnitLoadState {
    #[default]
    Loaded,
    NotFound,
    BadSetting,
    Error,
    Masked,
    Merged,
    Stub,
    Unknown,
}

impl UnitLoadState {
    pub fn color(self) -> Color {
        match self {
            Self::Loaded => Color::Green,
            Self::NotFound => Color::Yellow,
            Self::BadSetting | Self::Error | Self::Masked => Color::Red,
            Self::Merged | Self::Stub | Self::Unknown => Color::White,
        }
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    Eq,
    PartialEq,
    Hash,
    Ord,
    PartialOrd,
    Default,
    Display,
    EnumString,
    AsRefStr,
    IntoStaticStr,
    VariantNames,
)]
#[strum(serialize_all = "kebab-case")]
pub enum UnitActiveState {
    #[default]
    Active,
    Inactive,
    Failed,
    Activating,
    Deactivating,
    Maintenance,
    Reloading,
    Unknown,
}

impl UnitActiveState {
    pub fn color(self) -> Color {
        match self {
            Self::Active => Color::Green,
            Self::Failed => Color::Red,
            Self::Inactive => Color::Gray,
            Self::Activating | Self::Reloading => Color::Yellow,
            Self::Deactivating => Color::LightYellow,
            Self::Maintenance => Color::Magenta,
            Self::Unknown => Color::White,
        }
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    Eq,
    PartialEq,
    Hash,
    Ord,
    PartialOrd,
    Default,
    Display,
    EnumString,
    AsRefStr,
    IntoStaticStr,
    VariantNames,
)]
#[strum(serialize_all = "kebab-case")]
pub enum UnitEnablementState {
    #[default]
    Enabled,
    EnabledRuntime,
    Linked,
    LinkedRuntime,
    Masked,
    MaskedRuntime,
    Static,
    Disabled,
    DisabledRuntime,
    Invalid,
    Indirect,
    Alias,
    Generated,
    Transient,
    Unknown,
}

impl UnitEnablementState {
    pub fn color(self) -> Color {
        match self {
            Self::Enabled | Self::EnabledRuntime => Color::Green,
            Self::Static
            | Self::Generated
            | Self::Alias
            | Self::Indirect
            | Self::Linked
            | Self::LinkedRuntime => Color::Cyan,
            Self::Disabled | Self::DisabledRuntime => Color::Gray,
            Self::Masked | Self::MaskedRuntime | Self::Invalid => Color::Red,
            Self::Transient | Self::Unknown => Color::Yellow,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_type_round_trips_known_values_and_falls_back_to_unknown() {
        let cases = [
            (UnitType::Unknown, "unknown"),
            (UnitType::Service, "service"),
            (UnitType::Socket, "socket"),
            (UnitType::Target, "target"),
            (UnitType::Device, "device"),
            (UnitType::Mount, "mount"),
            (UnitType::Automount, "automount"),
            (UnitType::Timer, "timer"),
            (UnitType::Path, "path"),
            (UnitType::Slice, "slice"),
            (UnitType::Scope, "scope"),
            (UnitType::Swap, "swap"),
        ];

        for (unit_type, label) in cases {
            assert_eq!(unit_type.as_ref(), label);
            assert_eq!(label.parse::<UnitType>().unwrap(), unit_type);
        }

        assert_eq!(
            "weird".parse::<UnitType>().unwrap_or(UnitType::Unknown),
            UnitType::Unknown
        );
        assert_eq!(UnitType::from_unit_name("ssh"), UnitType::Unknown);
        assert_eq!(UnitType::from_unit_name("ssh.weird"), UnitType::Unknown);
        assert_eq!(
            UnitType::from_unit_name("foo.bar.service"),
            UnitType::Service
        );
    }

    #[test]
    fn state_labels_include_unknown_fallbacks() {
        assert_eq!(UnitLoadState::Unknown.as_ref(), "unknown");
        assert_eq!(UnitLoadState::Unknown.color(), Color::White);
        assert_eq!(UnitActiveState::Unknown.as_ref(), "unknown");
        assert_eq!(UnitActiveState::Unknown.color(), Color::White);
        assert_eq!(UnitEnablementState::Unknown.as_ref(), "unknown");
        assert_eq!(UnitEnablementState::Unknown.color(), Color::Yellow);
    }
}
