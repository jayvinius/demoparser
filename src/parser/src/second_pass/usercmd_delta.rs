//! Decoder for `CMsgServerUserCmd.delta_data` payloads emitted by CS2's
//! `codegen_delta_encoder`.
//!
//! Singular fields retain protobuf wire encoding except for wire type 7,
//! which resets a field to its declared default. The repeated input-history
//! and subtick fields use the replacement-list encoding observed in current
//! CS2 demos. Unknown or malformed operations fail the whole delta so callers
//! can keep the previous per-player baseline unchanged.

use csgoproto::CBaseUserCmdExecutionNotes;
use csgoproto::CInButtonStatePb;
use csgoproto::CMsgQAngle;
use csgoproto::CsgoUserCmdPb;
use prost::Message;
use std::fmt;

/// Why a delta could not be applied. The caller keeps the previous baseline either way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum DeltaError {
    /// Truncated varint or length, zero field number, or an invalid wire type.
    Malformed(&'static str),
    /// A wire-type-7 reset names a field this decoder has no schema for.
    UnknownResetField { schema: &'static str, field: u64 },
    /// The repeated-field operation stream broke the expected grammar.
    Repeated { schema: &'static str, reason: &'static str },
    /// prost rejected the sanitized bytes.
    Decode(&'static str),
}

impl fmt::Display for DeltaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed(what) => write!(f, "malformed: {what}"),
            Self::UnknownResetField { schema, field } => write!(f, "reset of unknown field {schema}.{field}"),
            Self::Repeated { schema, reason } => write!(f, "repeated {schema}: {reason}"),
            Self::Decode(what) => write!(f, "decode failed: {what}"),
        }
    }
}

#[derive(Clone, PartialEq, Message)]
struct DeltaBaseUserCmdPb {
    #[prost(int32, optional, tag = "1")]
    legacy_command_number: Option<i32>,
    #[prost(int32, optional, tag = "2")]
    client_tick: Option<i32>,
    #[prost(uint32, optional, tag = "17")]
    prediction_offset_ticks_x256: Option<u32>,
    #[prost(message, optional, tag = "3")]
    buttons_pb: Option<CInButtonStatePb>,
    #[prost(message, optional, tag = "4")]
    viewangles: Option<CMsgQAngle>,
    #[prost(float, optional, tag = "5")]
    forwardmove: Option<f32>,
    #[prost(float, optional, tag = "6")]
    leftmove: Option<f32>,
    #[prost(float, optional, tag = "7")]
    upmove: Option<f32>,
    #[prost(int32, optional, tag = "8")]
    impulse: Option<i32>,
    #[prost(int32, optional, tag = "9")]
    weaponselect: Option<i32>,
    #[prost(int32, optional, tag = "10")]
    random_seed: Option<i32>,
    #[prost(int32, optional, tag = "11")]
    mousedx: Option<i32>,
    #[prost(int32, optional, tag = "12")]
    mousedy: Option<i32>,
    #[prost(uint32, optional, tag = "14")]
    pawn_entity_handle: Option<u32>,
    #[prost(bytes = "bytes", repeated, tag = "18")]
    subtick_moves_delta: Vec<prost::bytes::Bytes>,
    #[prost(bytes = "bytes", optional, tag = "19")]
    move_crc: Option<prost::bytes::Bytes>,
    #[prost(uint32, optional, tag = "20")]
    consumed_server_angle_changes: Option<u32>,
    #[prost(int32, optional, tag = "21")]
    cmd_flags: Option<i32>,
    #[prost(bytes = "bytes", optional, tag = "22")]
    execution_notes: Option<prost::bytes::Bytes>,
}

#[derive(Clone, PartialEq, Message)]
struct DeltaCsgoUserCmdPb {
    #[prost(message, optional, tag = "1")]
    base: Option<DeltaBaseUserCmdPb>,
    #[prost(bytes = "bytes", repeated, tag = "2")]
    input_history_delta: Vec<prost::bytes::Bytes>,
    #[prost(int32, optional, tag = "6")]
    attack1_start_history_index: Option<i32>,
    #[prost(int32, optional, tag = "7")]
    attack2_start_history_index: Option<i32>,
    #[prost(bool, optional, tag = "9")]
    left_hand_desired: Option<bool>,
    #[prost(bool, optional, tag = "11")]
    is_predicting_body_shot_fx: Option<bool>,
    #[prost(bool, optional, tag = "12")]
    is_predicting_head_shot_fx: Option<bool>,
    #[prost(bool, optional, tag = "13")]
    is_predicting_kill_ragdolls: Option<bool>,
}

fn read_varint(bytes: &mut &[u8]) -> Option<u64> {
    let mut value = 0_u64;
    for shift in (0..70).step_by(7) {
        let (&byte, rest) = bytes.split_first()?;
        *bytes = rest;
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Some(value);
        }
    }
    None
}

fn write_varint(mut value: u64, out: &mut Vec<u8>) {
    while value >= 0x80 {
        out.push((value as u8 & 0x7f) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

#[derive(Clone, Copy)]
enum MessageSchema {
    CsgoUserCmd,
    BaseUserCmd,
    Buttons,
    QAngle,
    Vector,
    Interp,
    InterpCl,
    InputHistory,
    SubtickMove,
}

impl MessageSchema {
    fn name(self) -> &'static str {
        match self {
            Self::CsgoUserCmd => "CSGOUserCmdPB",
            Self::BaseUserCmd => "CBaseUserCmdPB",
            Self::Buttons => "CInButtonStatePB",
            Self::QAngle => "CMsgQAngle",
            Self::Vector => "CMsgVector",
            Self::Interp => "CSGOInterpolationInfoPB",
            Self::InterpCl => "CSGOInterpolationInfoPB_CL",
            Self::InputHistory => "CSGOInputHistoryEntryPB",
            Self::SubtickMove => "CSubtickMoveStep",
        }
    }

    fn field_wire_type(self, field: u64) -> Option<u8> {
        match self {
            Self::CsgoUserCmd => match field {
                1 | 2 => Some(2),
                6 | 7 | 9 | 11 | 12 | 13 => Some(0),
                _ => None,
            },
            Self::BaseUserCmd => match field {
                1 | 2 | 8 | 9 | 10 | 11 | 12 | 14 | 17 | 20 | 21 => Some(0),
                3 | 4 | 18 | 19 | 22 => Some(2),
                5 | 6 | 7 => Some(5),
                _ => None,
            },
            Self::Buttons => match field {
                1..=3 => Some(0),
                _ => None,
            },
            Self::QAngle => match field {
                1..=3 => Some(5),
                _ => None,
            },
            Self::Vector => match field {
                1..=4 => Some(5),
                _ => None,
            },
            Self::Interp => match field {
                1 | 2 => Some(0),
                3 => Some(5),
                _ => None,
            },
            Self::InterpCl => match field {
                3 => Some(5),
                _ => None,
            },
            Self::InputHistory => match field {
                2 | 12..=15 | 66..=69 => Some(2),
                4 | 6 | 64 | 65 => Some(0),
                5 | 7 => Some(5),
                _ => None,
            },
            Self::SubtickMove => match field {
                1 | 2 => Some(0),
                3 | 4 | 5 | 8 | 9 => Some(5),
                _ => None,
            },
        }
    }

    fn child(self, field: u64) -> Option<Self> {
        match (self, field) {
            (Self::CsgoUserCmd, 1) => Some(Self::BaseUserCmd),
            (Self::BaseUserCmd, 3) => Some(Self::Buttons),
            (Self::BaseUserCmd, 4) => Some(Self::QAngle),
            (Self::InputHistory, 2 | 69) => Some(Self::QAngle),
            (Self::InputHistory, 12) => Some(Self::InterpCl),
            (Self::InputHistory, 13..=15) => Some(Self::Interp),
            (Self::InputHistory, 66..=68) => Some(Self::Vector),
            _ => None,
        }
    }

    fn reset_fields(self) -> &'static [(u64, u8)] {
        match self {
            Self::CsgoUserCmd => &[(1, 2), (2, 2), (6, 0), (7, 0), (9, 0), (11, 0), (12, 0), (13, 0)],
            Self::BaseUserCmd => &[
                (1, 0),
                (2, 0),
                (3, 2),
                (4, 2),
                (5, 5),
                (6, 5),
                (7, 5),
                (8, 0),
                (9, 0),
                (10, 0),
                (11, 0),
                (12, 0),
                (14, 0),
                (17, 0),
                (18, 2),
                (19, 2),
                (20, 0),
                (21, 0),
                (22, 2),
            ],
            Self::Buttons => &[(1, 0), (2, 0), (3, 0)],
            Self::QAngle => &[(1, 5), (2, 5), (3, 5)],
            Self::Vector => &[(1, 5), (2, 5), (3, 5), (4, 5)],
            Self::Interp => &[(1, 0), (2, 0), (3, 5)],
            Self::InterpCl => &[(3, 5)],
            Self::InputHistory => &[(2, 2), (4, 0), (5, 5), (6, 0), (7, 5), (64, 0), (65, 0)],
            Self::SubtickMove => &[(1, 0), (2, 0), (3, 5), (4, 5), (5, 5), (8, 5), (9, 5)],
        }
    }

    fn write_default(self, field: u64, wire_type: u8, out: &mut Vec<u8>) -> Option<()> {
        match wire_type {
            0 => {
                let value = match (self, field) {
                    (Self::CsgoUserCmd, 6 | 7) | (Self::InputHistory, 65) | (Self::Interp, 1 | 2) => u64::MAX,
                    (Self::BaseUserCmd, 14) => 0x00ff_ffff,
                    _ => 0,
                };
                write_varint(value, out);
            }
            1 => out.extend_from_slice(&[0; 8]),
            2 => {
                let nested = match self.child(field) {
                    Some(child) => child.explicit_defaults()?,
                    None => Vec::new(),
                };
                write_varint(nested.len() as u64, out);
                out.extend_from_slice(&nested);
            }
            5 => out.extend_from_slice(&[0; 4]),
            _ => return None,
        }
        Some(())
    }

    fn explicit_defaults(self) -> Option<Vec<u8>> {
        let mut out = Vec::new();
        for &(field, wire_type) in self.reset_fields() {
            write_varint((field << 3) | u64::from(wire_type), &mut out);
            self.write_default(field, wire_type, &mut out)?;
        }
        Some(out)
    }
}

const TRUNCATED: DeltaError = DeltaError::Malformed("truncated");

/// Elements of repeated fields record which of their own fields a delta reset, so they can be
/// cleared to `None` after the merge. Writing the default alone is not enough for message
/// fields: merging an empty or default sub-message leaves the old value in place.
trait ClearField {
    fn clear_field(&mut self, field: u64);
}

impl ClearField for csgoproto::CsgoInputHistoryEntryPb {
    fn clear_field(&mut self, field: u64) {
        match field {
            2 => self.view_angles = None,
            4 => self.render_tick_count = None,
            5 => self.render_tick_fraction = None,
            6 => self.player_tick_count = None,
            7 => self.player_tick_fraction = None,
            12 => self.cl_interp = None,
            13 => self.sv_interp0 = None,
            14 => self.sv_interp1 = None,
            15 => self.player_interp = None,
            64 => self.frame_number = None,
            65 => self.target_ent_index = None,
            66 => self.shoot_position = None,
            67 => self.target_head_pos_check = None,
            68 => self.target_abs_pos_check = None,
            69 => self.target_abs_ang_check = None,
            _ => {}
        }
    }
}

impl ClearField for csgoproto::CSubtickMoveStep {
    fn clear_field(&mut self, field: u64) {
        match field {
            1 => self.button = None,
            2 => self.pressed = None,
            3 => self.when = None,
            4 => self.analog_forward_delta = None,
            5 => self.analog_left_delta = None,
            8 => self.pitch_delta = None,
            9 => self.yaw_delta = None,
            _ => {}
        }
    }
}

/// Rewrites a delta message into standard protobuf: wire-type-7 resets become explicit default
/// values. Field numbers reset at this level (not in nested messages) are pushed to `resets`.
fn sanitize_message(
    mut bytes: &[u8],
    schema: MessageSchema,
    mut resets: Option<&mut Vec<u64>>,
) -> Result<Vec<u8>, DeltaError> {
    let mut out = Vec::with_capacity(bytes.len());
    while !bytes.is_empty() {
        let key = read_varint(&mut bytes).ok_or(TRUNCATED)?;
        let field = key >> 3;
        let wire_type = (key & 0x07) as u8;
        if field == 0 {
            return Err(DeltaError::Malformed("field number 0"));
        }

        if wire_type == 7 {
            if let Some(resets) = resets.as_deref_mut() {
                resets.push(field);
            }
            let normal_wire_type = schema
                .field_wire_type(field)
                .ok_or(DeltaError::UnknownResetField { schema: schema.name(), field })?;
            write_varint((field << 3) | u64::from(normal_wire_type), &mut out);
            schema
                .write_default(field, normal_wire_type, &mut out)
                .ok_or(DeltaError::Malformed("no default for reset field"))?;
            continue;
        }

        write_varint(key, &mut out);
        match wire_type {
            0 => {
                let value = read_varint(&mut bytes).ok_or(TRUNCATED)?;
                write_varint(value, &mut out);
            }
            1 => {
                let (value, rest) = bytes.split_at_checked(8).ok_or(TRUNCATED)?;
                out.extend_from_slice(value);
                bytes = rest;
            }
            2 => {
                let length = usize::try_from(read_varint(&mut bytes).ok_or(TRUNCATED)?).map_err(|_| TRUNCATED)?;
                let (value, rest) = bytes.split_at_checked(length).ok_or(TRUNCATED)?;
                let value = if let Some(child) = schema.child(field) {
                    sanitize_message(value, child, None)?
                } else {
                    value.to_vec()
                };
                write_varint(value.len() as u64, &mut out);
                out.extend_from_slice(&value);
                bytes = rest;
            }
            5 => {
                let (value, rest) = bytes.split_at_checked(4).ok_or(TRUNCATED)?;
                out.extend_from_slice(value);
                bytes = rest;
            }
            _ => return Err(DeltaError::Malformed("invalid wire type")),
        }
    }
    Ok(out)
}

/// Reconstructs the codegen-delta list from its per-player-slot baseline.
/// The leading wire-type 7 key declares the target length; following field
/// numbers are zero-based element indices and may be sparse.
fn decode_repeated<M>(
    payloads: &[prost::bytes::Bytes],
    schema: MessageSchema,
    previous: &[M],
) -> Result<Vec<M>, DeltaError>
where
    M: Message + Default + Clone + ClearField,
{
    let fail = |reason| DeltaError::Repeated { schema: schema.name(), reason };
    let mut resets = Vec::new();
    let mut messages = previous.to_vec();
    let mut declared_count = None;
    for payload in payloads {
        let mut bytes = payload.as_ref();
        if !bytes.is_empty() {
            let mut after_marker = bytes;
            let marker = read_varint(&mut after_marker).ok_or(TRUNCATED)?;
            if marker & 0x07 == 7 {
                if declared_count.is_some() {
                    return Err(fail("second length marker"));
                }
                let count = usize::try_from(marker >> 3).map_err(|_| TRUNCATED)?;
                messages.resize(count, M::default());
                declared_count = Some(count);
                bytes = after_marker;
            }
        }
        while !bytes.is_empty() {
            let key = read_varint(&mut bytes).ok_or(TRUNCATED)?;
            if key & 0x07 != 2 {
                return Err(fail("element op is not length-delimited"));
            }
            let index = usize::try_from(key >> 3).map_err(|_| TRUNCATED)?;
            let length = usize::try_from(read_varint(&mut bytes).ok_or(TRUNCATED)?).map_err(|_| TRUNCATED)?;
            let (message, rest) = bytes.split_at_checked(length).ok_or(TRUNCATED)?;
            resets.clear();
            let message = sanitize_message(message, schema, Some(&mut resets))?;
            let element = messages.get_mut(index).ok_or(fail("index out of bounds"))?;
            element.merge(message.as_slice()).map_err(|_| DeltaError::Decode(schema.name()))?;
            for &field in &resets {
                element.clear_field(field);
            }
            bytes = rest;
        }
    }
    Ok(messages)
}

fn replace_if_some<T>(target: &mut Option<T>, value: Option<T>) {
    if let Some(value) = value {
        *target = Some(value);
    }
}

fn merge_buttons(target: &mut Option<CInButtonStatePb>, delta: CInButtonStatePb) {
    let target = target.get_or_insert_with(|| Default::default());
    replace_if_some(&mut target.buttonstate1, delta.buttonstate1);
    replace_if_some(&mut target.buttonstate2, delta.buttonstate2);
    replace_if_some(&mut target.buttonstate3, delta.buttonstate3);
}

fn merge_qangle(target: &mut Option<CMsgQAngle>, delta: CMsgQAngle) {
    let target = target.get_or_insert_with(|| Default::default());
    replace_if_some(&mut target.x, delta.x);
    replace_if_some(&mut target.y, delta.y);
    replace_if_some(&mut target.z, delta.z);
}

pub(super) fn apply_delta(baseline: &CsgoUserCmdPb, delta_data: &[u8]) -> Result<CsgoUserCmdPb, DeltaError> {
    let sanitized = sanitize_message(delta_data, MessageSchema::CsgoUserCmd, None)?;
    let delta = DeltaCsgoUserCmdPb::decode(sanitized.as_slice()).map_err(|_| DeltaError::Decode("CSGOUserCmdPB"))?;
    let mut next = baseline.clone();

    if !delta.input_history_delta.is_empty() {
        next.input_history = decode_repeated(&delta.input_history_delta, MessageSchema::InputHistory, &next.input_history)?;
    }
    replace_if_some(&mut next.attack1_start_history_index, delta.attack1_start_history_index);
    replace_if_some(&mut next.attack2_start_history_index, delta.attack2_start_history_index);
    replace_if_some(&mut next.left_hand_desired, delta.left_hand_desired);
    replace_if_some(&mut next.is_predicting_body_shot_fx, delta.is_predicting_body_shot_fx);
    replace_if_some(&mut next.is_predicting_head_shot_fx, delta.is_predicting_head_shot_fx);
    replace_if_some(&mut next.is_predicting_kill_ragdolls, delta.is_predicting_kill_ragdolls);

    if let Some(delta_base) = delta.base {
        let base = next.base.get_or_insert(Default::default());
        replace_if_some(&mut base.legacy_command_number, delta_base.legacy_command_number);
        replace_if_some(&mut base.client_tick, delta_base.client_tick);
        replace_if_some(&mut base.prediction_offset_ticks_x256, delta_base.prediction_offset_ticks_x256);
        if let Some(buttons) = delta_base.buttons_pb {
            merge_buttons(&mut base.buttons_pb, buttons);
        }
        if let Some(viewangles) = delta_base.viewangles {
            merge_qangle(&mut base.viewangles, viewangles);
        }
        replace_if_some(&mut base.forwardmove, delta_base.forwardmove);
        replace_if_some(&mut base.leftmove, delta_base.leftmove);
        replace_if_some(&mut base.upmove, delta_base.upmove);
        replace_if_some(&mut base.impulse, delta_base.impulse);
        replace_if_some(&mut base.weaponselect, delta_base.weaponselect);
        replace_if_some(&mut base.random_seed, delta_base.random_seed);
        replace_if_some(&mut base.mousedx, delta_base.mousedx);
        replace_if_some(&mut base.mousedy, delta_base.mousedy);
        replace_if_some(&mut base.pawn_entity_handle, delta_base.pawn_entity_handle);
        replace_if_some(&mut base.move_crc, delta_base.move_crc);
        replace_if_some(&mut base.consumed_server_angle_changes, delta_base.consumed_server_angle_changes);
        replace_if_some(&mut base.cmd_flags, delta_base.cmd_flags);
        if let Some(notes) = delta_base.execution_notes {
            base.execution_notes =
                Some(CBaseUserCmdExecutionNotes::decode(notes).map_err(|_| DeltaError::Decode("ExecutionNotes"))?);
        }
        if !delta_base.subtick_moves_delta.is_empty() {
            base.subtick_moves = decode_repeated(&delta_base.subtick_moves_delta, MessageSchema::SubtickMove, &base.subtick_moves)?;
        }
    }

    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_july_usercmd_fields_and_repeated_subticks() {
        let bytes = [
            0x0a, 0x40, 0x10, 0xa5, 0x54, 0x1a, 0x06, 0x08, 0x90, 0x08, 0x10, 0x80, 0x08, 0x22, 0x0a, 0x0d, 0x87, 0x85, 0x29, 0x40, 0x15, 0x36, 0x07, 0xc7,
            0x42, 0x35, 0x00, 0x00, 0x80, 0xbf, 0x50, 0xf8, 0xfb, 0xa7, 0xf7, 0x07, 0x58, 0x51, 0x60, 0x06, 0x92, 0x01, 0x17, 0x0f, 0x02, 0x14, 0x08, 0x80,
            0x08, 0x10, 0x01, 0x1d, 0x00, 0x00, 0xd8, 0x3e, 0x45, 0x3c, 0x4e, 0x11, 0xbf, 0x4d, 0xf0, 0x6a, 0xd5, 0x40,
        ];
        let mut baseline = CsgoUserCmdPb::default();
        baseline.base = Some(Default::default());
        baseline.base.as_mut().unwrap().forwardmove = Some(0.75);
        baseline.base.as_mut().unwrap().viewangles = Some(CMsgQAngle {
            z: Some(17.0),
            ..Default::default()
        });

        let command = apply_delta(&baseline, &bytes).unwrap();
        let base = command.base.unwrap();
        let buttons = base.buttons_pb.unwrap();
        assert_eq!(buttons.buttonstate1, Some(0x410));
        assert_eq!(buttons.buttonstate2, Some(0x400));
        assert_eq!(base.forwardmove, Some(0.75));
        assert_eq!(base.leftmove, Some(-1.0));
        assert_eq!(base.viewangles.unwrap().z, Some(17.0));
        assert_eq!(base.subtick_moves.len(), 1);
        assert_eq!(base.subtick_moves[0].button(), 0x400);
        assert!(base.subtick_moves[0].pressed());
        assert!((base.subtick_moves[0].when() - 0.421875).abs() < f32::EPSILON);
    }

    #[test]
    fn repeated_marker_declares_multiple_elements() {
        let payload = prost::bytes::Bytes::from_static(&[
            0x17, 0x02, 0x09, 0x08, 0x01, 0x10, 0x01, 0x1d, 0x00, 0x00, 0xb0, 0x3e, 0x0a, 0x0a, 0x08, 0x80, 0x10, 0x10, 0x01, 0x1d, 0x00, 0x00, 0xb0, 0x3e,
        ]);

        let decoded = decode_repeated::<csgoproto::CSubtickMoveStep>(&[payload], MessageSchema::SubtickMove, &[]).unwrap();

        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].button, Some(1));
        assert_eq!(decoded[1].button, Some(2048));
    }

    #[test]
    fn repeated_delta_allows_sparse_indices_and_merges_element_baselines() {
        let previous = vec![
            csgoproto::CSubtickMoveStep::default(),
            csgoproto::CSubtickMoveStep {
                button: Some(2),
                pressed: Some(true),
                when: Some(0.25),
                ..Default::default()
            },
        ];
        let payload = prost::bytes::Bytes::from_static(&[0x17, 0x0a, 0x02, 0x08, 0x04]);

        let decoded = decode_repeated(&[payload], MessageSchema::SubtickMove, &previous).unwrap();

        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0], csgoproto::CSubtickMoveStep::default());
        assert_eq!(decoded[1].button, Some(4));
        assert_eq!(decoded[1].pressed, Some(true));
        assert_eq!(decoded[1].when, Some(0.25));
    }

    #[test]
    fn expands_nested_clear_markers_and_preserves_omitted_fields() {
        let bytes = [
            0x0a, 0x12, 0x10, 0xa6, 0x54, 0x1a, 0x01, 0x17, 0x50, 0xed, 0xf1, 0xc9, 0xdd, 0x03, 0x97, 0x01, 0xa8, 0x01, 0x80, 0x01,
        ];
        let mut baseline = CsgoUserCmdPb::default();
        let mut base = csgoproto::CBaseUserCmdPb::default();
        base.forwardmove = Some(1.0);
        base.buttons_pb = Some(CInButtonStatePb {
            buttonstate1: Some(1),
            buttonstate2: Some(2),
            buttonstate3: Some(3),
        });
        baseline.base = Some(base);

        let command = apply_delta(&baseline, &bytes).unwrap();
        let base = command.base.unwrap();
        assert_eq!(base.forwardmove, Some(1.0));
        let buttons = base.buttons_pb.unwrap();
        assert_eq!(buttons.buttonstate1, Some(1));
        assert_eq!(buttons.buttonstate2, Some(0));
        assert_eq!(buttons.buttonstate3, Some(3));
    }

    /// Wraps one input-history element op (index 0) in a top-level CSGOUserCmdPB delta.
    fn history_delta(element: &[u8]) -> Vec<u8> {
        let mut op = vec![0x02, element.len() as u8];
        op.extend_from_slice(element);
        let mut out = vec![0x12, op.len() as u8];
        out.extend_from_slice(&op);
        out
    }

    fn vector(x: f32, y: f32, z: f32) -> csgoproto::CMsgVector {
        csgoproto::CMsgVector {
            x: Some(x),
            y: Some(y),
            z: Some(z),
            w: None,
        }
    }

    #[test]
    fn reset_clears_message_field_in_history_element() {
        let mut baseline = CsgoUserCmdPb::default();
        baseline.input_history = vec![csgoproto::CsgoInputHistoryEntryPb {
            view_angles: Some(CMsgQAngle {
                x: Some(5.0),
                y: Some(6.0),
                z: None,
            }),
            target_head_pos_check: Some(vector(1.0, 2.0, 3.0)),
            ..Default::default()
        }];
        // Field 67 (target_head_pos_check) with wire type 7.
        let command = apply_delta(&baseline, &history_delta(&[0x9f, 0x04])).unwrap();
        let entry = &command.input_history[0];
        assert_eq!(entry.target_head_pos_check, None);
        assert_eq!(entry.view_angles.as_ref().unwrap().y, Some(6.0));
    }

    #[test]
    fn nested_reset_inside_vector_keeps_other_components() {
        let mut baseline = CsgoUserCmdPb::default();
        baseline.input_history = vec![csgoproto::CsgoInputHistoryEntryPb {
            shoot_position: Some(vector(1.0, 2.0, 3.0)),
            ..Default::default()
        }];
        // Field 66 (shoot_position): reset x, set y = 2.5. Used to fail the whole command.
        let element = [0x92, 0x04, 0x06, 0x0f, 0x15, 0x00, 0x00, 0x20, 0x40];
        let command = apply_delta(&baseline, &history_delta(&element)).unwrap();
        let shoot = command.input_history[0].shoot_position.as_ref().unwrap();
        assert_eq!(shoot.x(), 0.0);
        assert_eq!(shoot.y, Some(2.5));
        assert_eq!(shoot.z, Some(3.0));
    }

    #[test]
    fn interp_reset_uses_minus_one_default() {
        let mut baseline = CsgoUserCmdPb::default();
        baseline.input_history = vec![csgoproto::CsgoInputHistoryEntryPb {
            sv_interp0: Some(csgoproto::CsgoInterpolationInfoPb {
                src_tick: Some(5),
                dst_tick: Some(6),
                frac: Some(0.5),
            }),
            ..Default::default()
        }];
        // Field 13 (sv_interp0): reset src_tick.
        let command = apply_delta(&baseline, &history_delta(&[0x6a, 0x01, 0x0f])).unwrap();
        let interp = command.input_history[0].sv_interp0.as_ref().unwrap();
        assert_eq!(interp.src_tick, Some(-1));
        assert_eq!(interp.dst_tick, Some(6));
        assert_eq!(interp.frac, Some(0.5));
    }

    #[test]
    fn wire_seven_uses_declared_nonzero_defaults() {
        let mut baseline = CsgoUserCmdPb::default();
        baseline.attack1_start_history_index = Some(4);
        baseline.base = Some(csgoproto::CBaseUserCmdPb {
            pawn_entity_handle: Some(123),
            ..Default::default()
        });

        let command = apply_delta(&baseline, &[0x37, 0x0a, 0x01, 0x77]).unwrap();
        assert_eq!(command.attack1_start_history_index, Some(-1));
        assert_eq!(command.base.unwrap().pawn_entity_handle, Some(0x00ff_ffff));
    }
}
