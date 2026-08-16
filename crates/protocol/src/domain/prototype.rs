//! Dev Mode annotations, prototype reactions, and capability-gated motion contracts.

use super::{
    Color, ConnectionId, ItemResult, NodeId, ObservationWindow, ReturnedList, Selector, Truncation,
};
use schemars::JsonSchema;
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{Error as _, MapAccess, Visitor},
};
use std::fmt;

macro_rules! prototype_input {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        pub struct $name {
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub connection_id: Option<ConnectionId>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub selector: Option<Selector>,
        }
    };
}

prototype_input!(GetDevModeDataInput);
prototype_input!(GetReactionsInput);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnnotationValue {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category_id: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnnotationCategory {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DevResource {
    pub name: String,
    pub uri: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DevModeNodeData {
    pub node_id: NodeId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description_markdown: Option<String>,
    pub annotations: ReturnedList<AnnotationValue>,
    pub annotation_categories: ReturnedList<AnnotationCategory>,
    pub documentation: ReturnedList<DevResource>,
    pub dev_resources: ReturnedList<DevResource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_node_id: Option<NodeId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inherited_from_node_id: Option<NodeId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GetDevModeDataResult {
    pub items: ReturnedList<ItemResult<DevModeNodeData>>,
    pub truncated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truncation: Option<Truncation>,
    pub observation: ObservationWindow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ReactionTrigger {
    Click,
    Drag,
    Hover,
    Press,
    KeyDown,
    AfterDelay,
    MouseEnter,
    MouseLeave,
    MouseUp,
    MouseDown,
    MediaHit,
    MediaEnd,
}

#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ReactionAction {
    Navigate {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        destination_id: Option<NodeId>,
    },
    OpenOverlay {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        destination_id: Option<NodeId>,
    },
    SwapOverlay {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        destination_id: Option<NodeId>,
    },
    CloseOverlay,
    Back,
    ChangeTo {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        destination_id: Option<NodeId>,
    },
    ScrollTo {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        destination_id: Option<NodeId>,
    },
    OpenLink {
        uri: String,
    },
    SetVariable {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        variable_id: Option<String>,
    },
    SetVariableMode {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        variable_collection_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        variable_mode_id: Option<String>,
    },
    Conditional,
    UpdateMediaRuntime {
        media_action: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        destination_id: Option<NodeId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        amount_to_skip: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        new_timestamp: Option<f64>,
    },
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
enum ReactionActionTag {
    Navigate,
    OpenOverlay,
    SwapOverlay,
    CloseOverlay,
    Back,
    ChangeTo,
    ScrollTo,
    OpenLink,
    SetVariable,
    SetVariableMode,
    Conditional,
    UpdateMediaRuntime,
}

#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "camelCase")]
enum ReactionActionField {
    Type,
    DestinationId,
    Uri,
    VariableId,
    VariableCollectionId,
    VariableModeId,
    MediaAction,
    AmountToSkip,
    NewTimestamp,
}

impl<'de> Deserialize<'de> for ReactionAction {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(ReactionActionVisitor)
    }
}

struct ReactionActionVisitor;

impl<'de> Visitor<'de> for ReactionActionVisitor {
    type Value = ReactionAction;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a closed prototype reaction action")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut tag = None;
        let mut destination_id = None;
        let mut uri = None;
        let mut variable_id = None;
        let mut variable_collection_id = None;
        let mut variable_mode_id = None;
        let mut media_action = None;
        let mut amount_to_skip = None;
        let mut new_timestamp = None;
        while let Some(field) = map.next_key::<ReactionActionField>()? {
            match field {
                ReactionActionField::Type => {
                    if tag
                        .replace(map.next_value::<ReactionActionTag>()?)
                        .is_some()
                    {
                        return Err(A::Error::duplicate_field("type"));
                    }
                }
                ReactionActionField::DestinationId => {
                    if destination_id
                        .replace(map.next_value::<NodeId>()?)
                        .is_some()
                    {
                        return Err(A::Error::duplicate_field("destinationId"));
                    }
                }
                ReactionActionField::Uri => {
                    if uri.replace(map.next_value::<String>()?).is_some() {
                        return Err(A::Error::duplicate_field("uri"));
                    }
                }
                ReactionActionField::VariableId => {
                    if variable_id.replace(map.next_value::<String>()?).is_some() {
                        return Err(A::Error::duplicate_field("variableId"));
                    }
                }
                ReactionActionField::VariableCollectionId => {
                    if variable_collection_id
                        .replace(map.next_value::<String>()?)
                        .is_some()
                    {
                        return Err(A::Error::duplicate_field("variableCollectionId"));
                    }
                }
                ReactionActionField::VariableModeId => {
                    if variable_mode_id
                        .replace(map.next_value::<String>()?)
                        .is_some()
                    {
                        return Err(A::Error::duplicate_field("variableModeId"));
                    }
                }
                ReactionActionField::MediaAction => {
                    if media_action.replace(map.next_value::<String>()?).is_some() {
                        return Err(A::Error::duplicate_field("mediaAction"));
                    }
                }
                ReactionActionField::AmountToSkip => {
                    if amount_to_skip.replace(map.next_value::<f64>()?).is_some() {
                        return Err(A::Error::duplicate_field("amountToSkip"));
                    }
                }
                ReactionActionField::NewTimestamp => {
                    if new_timestamp.replace(map.next_value::<f64>()?).is_some() {
                        return Err(A::Error::duplicate_field("newTimestamp"));
                    }
                }
            }
        }
        let tag = tag.ok_or_else(|| A::Error::missing_field("type"))?;
        match tag {
            ReactionActionTag::Navigate
            | ReactionActionTag::OpenOverlay
            | ReactionActionTag::SwapOverlay
            | ReactionActionTag::ChangeTo
            | ReactionActionTag::ScrollTo => {
                if uri.is_some() {
                    return Err(A::Error::custom("destination action cannot contain a URI"));
                }
                Ok(match tag {
                    ReactionActionTag::Navigate => ReactionAction::Navigate { destination_id },
                    ReactionActionTag::OpenOverlay => {
                        ReactionAction::OpenOverlay { destination_id }
                    }
                    ReactionActionTag::SwapOverlay => {
                        ReactionAction::SwapOverlay { destination_id }
                    }
                    ReactionActionTag::ChangeTo => ReactionAction::ChangeTo { destination_id },
                    ReactionActionTag::ScrollTo => ReactionAction::ScrollTo { destination_id },
                    _ => unreachable!("destination tag was matched above"),
                })
            }
            ReactionActionTag::OpenLink => {
                if destination_id.is_some() {
                    return Err(A::Error::custom(
                        "open-link action cannot contain destinationId",
                    ));
                }
                Ok(ReactionAction::OpenLink {
                    uri: uri.ok_or_else(|| A::Error::missing_field("uri"))?,
                })
            }
            ReactionActionTag::CloseOverlay | ReactionActionTag::Back => {
                if destination_id.is_some() || uri.is_some() {
                    return Err(A::Error::custom(
                        "payload-free reaction action contains extra fields",
                    ));
                }
                if matches!(tag, ReactionActionTag::CloseOverlay) {
                    Ok(ReactionAction::CloseOverlay)
                } else {
                    Ok(ReactionAction::Back)
                }
            }
            ReactionActionTag::SetVariable => Ok(ReactionAction::SetVariable { variable_id }),
            ReactionActionTag::SetVariableMode => Ok(ReactionAction::SetVariableMode {
                variable_collection_id,
                variable_mode_id,
            }),
            ReactionActionTag::Conditional => Ok(ReactionAction::Conditional),
            ReactionActionTag::UpdateMediaRuntime => Ok(ReactionAction::UpdateMediaRuntime {
                media_action: media_action.ok_or_else(|| A::Error::missing_field("mediaAction"))?,
                destination_id,
                amount_to_skip,
                new_timestamp,
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum OverlayPositionType {
    Center,
    TopLeft,
    TopCenter,
    TopRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum OverlayBackgroundInteraction {
    None,
    CloseOnClickOutside,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum OverlayBackground {
    None,
    SolidColor { color: Color },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReactionOverlay {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relative_position: Option<Vector2>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position_type: Option<OverlayPositionType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<OverlayBackground>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background_interaction: Option<OverlayBackgroundInteraction>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Reaction {
    pub trigger: ReactionTrigger,
    pub action: ReactionAction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition_id: Option<String>,
    // Host transition.duration. Official reactions example: seconds (0.2). Not converted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition_duration: Option<f64>,
    pub destination_accessible: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overlay: Option<ReactionOverlay>,
    // Host AFTER_TIMEOUT.timeout in seconds (live: UI 800ms → 0.8). Not converted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<f64>,
    // Host mouse-trigger delay. Same Trigger surface; not live-measured. Not converted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delay: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_codes: Option<Vec<u32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
    // Host ON_MEDIA_HIT.mediaHitTime (docs alias timestamp). Official notes: seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_hit_time: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NodeReactions {
    pub node_id: NodeId,
    pub reactions: ReturnedList<Reaction>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GetReactionsResult {
    pub items: ReturnedList<ItemResult<NodeReactions>>,
    pub truncated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truncation: Option<Truncation>,
    pub observation: ObservationWindow,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GetMotionInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection_id: Option<ConnectionId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selector: Option<Selector>,
    #[serde(default)]
    pub include_available_styles: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum MotionEasingType {
    #[serde(rename = "LINEAR")]
    Linear,
    #[serde(rename = "EASE_IN")]
    EaseIn,
    #[serde(rename = "EASE_OUT")]
    EaseOut,
    #[serde(rename = "EASE_IN_AND_OUT")]
    EaseInAndOut,
    #[serde(rename = "EASE_IN_BACK")]
    EaseInBack,
    #[serde(rename = "EASE_OUT_BACK")]
    EaseOutBack,
    #[serde(rename = "EASE_IN_AND_OUT_BACK")]
    EaseInAndOutBack,
    #[serde(rename = "CUSTOM_CUBIC_BEZIER")]
    CustomCubicBezier,
    #[serde(rename = "GENTLE")]
    Gentle,
    #[serde(rename = "QUICK")]
    Quick,
    #[serde(rename = "BOUNCY")]
    Bouncy,
    #[serde(rename = "SLOW")]
    Slow,
    #[serde(rename = "CUSTOM_SPRING")]
    CustomSpring,
    #[serde(rename = "HOLD")]
    Hold,
    #[serde(rename = "VARIABLE_ALIAS")]
    VariableAlias,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CubicBezier {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NormalizedSpring {
    pub bounce: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MotionEasing {
    #[serde(rename = "type")]
    pub easing_type: MotionEasingType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub easing_function_cubic_bezier: Option<CubicBezier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub easing_function_spring: Option<NormalizedSpring>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Vector2 {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CircleValue {
    pub x: f64,
    pub y: f64,
    pub radius: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LineValue {
    pub x: f64,
    pub y: f64,
    pub x2: f64,
    pub y2: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CirclePointValue {
    pub x: f64,
    pub y: f64,
    pub radius: f64,
    pub angle: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ColorPointValue {
    pub x: f64,
    pub y: f64,
    pub color: Color,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum MotionKeyframeValue {
    #[serde(rename = "FLOAT")]
    Float { value: f64 },
    #[serde(rename = "COLOR")]
    Color { value: Color },
    #[serde(rename = "TEXT_DATA")]
    TextData { value: String },
    #[serde(rename = "VECTOR")]
    Vector { value: Vector2 },
    #[serde(rename = "BOOL")]
    Bool { value: bool },
    #[serde(rename = "CIRCLE")]
    Circle { value: CircleValue },
    #[serde(rename = "LINE")]
    Line { value: LineValue },
    #[serde(rename = "CIRCLE_POINT")]
    CirclePoint { value: CirclePointValue },
    #[serde(rename = "COLOR_POINT")]
    ColorPoint { value: ColorPointValue },
    #[serde(rename = "unsupported")]
    Unsupported { tag: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum AppliedStylePropValue {
    String(String),
    Boolean(bool),
    Number(f64),
    Easing(MotionEasing),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppliedStyleProp {
    pub name: String,
    pub value: AppliedStylePropValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AvailableStyleProp {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppliedAnimationStyle {
    pub id: String,
    pub style_id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeline_offset: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub props: Option<ReturnedList<AppliedStyleProp>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AvailableAnimationStyle {
    pub style_id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub props: Option<ReturnedList<AvailableStyleProp>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum IndexedCollection {
    Fills,
    Strokes,
    Effects,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum KeyframeField {
    Property {
        name: String,
    },
    IndexedItem {
        collection: IndexedCollection,
        index: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        field: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        property_id: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum KeyframeOperation {
    Set,
    Offset,
    Scale,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MotionKeyframe {
    pub id: String,
    pub timeline_position: f64,
    pub value: MotionKeyframeValue,
    pub easing: MotionEasing,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimationTrack {
    pub id: String,
    pub keyframe_operation: KeyframeOperation,
    pub keyframes: ReturnedList<MotionKeyframe>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimationBinding {
    pub field: KeyframeField,
    pub base_value: MotionKeyframeValue,
    pub timeline_duration: f64,
    pub tracks: ReturnedList<AnimationTrack>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ManualTrackBinding {
    pub field: KeyframeField,
    pub id: String,
    pub base_value: MotionKeyframeValue,
    pub keyframes: ReturnedList<MotionKeyframe>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MotionTimeline {
    pub id: String,
    pub duration: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NodeMotion {
    pub node_id: NodeId,
    pub animation_styles: ReturnedList<AppliedAnimationStyle>,
    pub animations: ReturnedList<AnimationBinding>,
    pub manual_keyframe_tracks: ReturnedList<ManualTrackBinding>,
    pub timelines: ReturnedList<MotionTimeline>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GetMotionResult {
    pub items: ReturnedList<ItemResult<NodeMotion>>,
    #[serde(default, skip_serializing_if = "ReturnedList::is_empty")]
    pub available_styles: ReturnedList<AvailableAnimationStyle>,
    pub truncated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truncation: Option<Truncation>,
    pub observation: ObservationWindow,
}
