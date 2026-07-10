use crate::model::{
    AdaptiveTriggerMode, AdaptiveTriggerPreset, AdaptiveTriggerProfile, HapticDemo,
};

use super::{AdaptiveTriggerEffect, HapticFrame};

pub fn demo_frames(demo: HapticDemo) -> Vec<HapticFrame> {
    match demo {
        HapticDemo::Click => vec![HapticFrame::symmetric(70, 45)],
        HapticDemo::Thump => vec![HapticFrame::symmetric(230, 170)],
        HapticDemo::Buzz => vec![
            HapticFrame::symmetric(170, 35),
            HapticFrame::symmetric(0, 25),
            HapticFrame::symmetric(190, 35),
            HapticFrame::symmetric(0, 25),
            HapticFrame::symmetric(210, 45),
        ],
        HapticDemo::Heartbeat => vec![
            HapticFrame::symmetric(210, 110),
            HapticFrame::symmetric(0, 120),
            HapticFrame::symmetric(255, 150),
        ],
        HapticDemo::Sweep => vec![
            HapticFrame::symmetric(25, 85),
            HapticFrame::symmetric(95, 85),
            HapticFrame::symmetric(155, 85),
            HapticFrame::symmetric(210, 120),
        ],
        HapticDemo::Impact => {
            vec![
                HapticFrame::symmetric(255, 140),
                HapticFrame::symmetric(0, 70),
            ]
        }
        HapticDemo::Tap => {
            vec![
                HapticFrame::symmetric(220, 140),
                HapticFrame::symmetric(0, 70),
            ]
        }
        HapticDemo::PulseTrain => vec![
            HapticFrame::symmetric(220, 90),
            HapticFrame::symmetric(0, 45),
            HapticFrame::symmetric(140, 90),
            HapticFrame::symmetric(0, 45),
            HapticFrame::symmetric(220, 90),
            HapticFrame::symmetric(0, 45),
            HapticFrame::symmetric(140, 90),
            HapticFrame::symmetric(0, 70),
        ],
    }
}

pub fn trigger_effect_for(preset: AdaptiveTriggerPreset, intensity: u8) -> AdaptiveTriggerEffect {
    let strength = trigger_strength(intensity);
    match preset {
        AdaptiveTriggerPreset::Off => AdaptiveTriggerEffect::off(),
        AdaptiveTriggerPreset::Bow => progressive_effect(2, 9, strength),
        AdaptiveTriggerPreset::MachineGun => vibration_effect(0, strength, 38),
        AdaptiveTriggerPreset::Pistol => weapon_effect(2, 5, strength),
        AdaptiveTriggerPreset::Rigid => resistance_effect(1, 9, strength),
        AdaptiveTriggerPreset::Brake => resistance_effect(5, 9, strength),
        AdaptiveTriggerPreset::Pulse => vibration_effect(1, strength, 16),
        AdaptiveTriggerPreset::Click => weapon_effect(1, 3, strength),
    }
}

pub fn trigger_effect_for_profile(profile: &AdaptiveTriggerProfile) -> AdaptiveTriggerEffect {
    match profile.mode {
        AdaptiveTriggerMode::Preset => trigger_effect_for(profile.preset, profile.intensity),
        AdaptiveTriggerMode::Resistance => resistance_effect(
            profile.start_position,
            profile.end_position.max(profile.start_position),
            trigger_strength(profile.intensity),
        ),
        AdaptiveTriggerMode::Vibration => vibration_effect(
            profile.start_position,
            trigger_strength(profile.intensity),
            profile.frequency.max(1),
        ),
    }
}

fn trigger_strength(intensity: u8) -> u8 {
    let scaled = 1 + (u16::from(intensity) * 7 / 255) as u8;
    scaled.clamp(1, 8)
}

fn resistance_effect(start: u8, end: u8, strength: u8) -> AdaptiveTriggerEffect {
    let mut zones = [0; 10];
    let start = start.min(9);
    let end = end.max(start).min(9);
    for zone in start..=end {
        zones[usize::from(zone)] = strength.clamp(1, 8);
    }
    feedback_effect(zones)
}

fn progressive_effect(start: u8, end: u8, max_strength: u8) -> AdaptiveTriggerEffect {
    let mut zones = [0; 10];
    let start = start.min(9);
    let end = end.min(9);
    let span = u16::from(end.saturating_sub(start)).max(1);
    for zone in start..=end {
        let step = u16::from(zone - start + 1);
        let strength =
            ((step * u16::from(max_strength.clamp(1, 8))) / (span + 1)).clamp(1, 8) as u8;
        zones[usize::from(zone)] = strength;
    }
    feedback_effect(zones)
}

fn feedback_effect(zones: [u8; 10]) -> AdaptiveTriggerEffect {
    let mut active_mask = 0u16;
    let mut packed_strengths = 0u32;
    for (index, strength) in zones.into_iter().enumerate() {
        if strength == 0 {
            continue;
        }

        let normalized = strength.clamp(1, 8) - 1;
        active_mask |= 1 << index;
        packed_strengths |= u32::from(normalized) << (index * 3);
    }

    AdaptiveTriggerEffect::from_bytes([
        0x21,
        active_mask as u8,
        (active_mask >> 8) as u8,
        packed_strengths as u8,
        (packed_strengths >> 8) as u8,
        (packed_strengths >> 16) as u8,
        (packed_strengths >> 24) as u8,
        0,
        0,
        0,
        0,
    ])
}

fn weapon_effect(start: u8, end: u8, strength: u8) -> AdaptiveTriggerEffect {
    let start = start.clamp(2, 7);
    let end = end.clamp(start + 1, 8);
    let zones = (1u16 << start) | (1u16 << end);
    AdaptiveTriggerEffect::from_bytes([
        0x25,
        zones as u8,
        (zones >> 8) as u8,
        strength.clamp(1, 8) - 1,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
    ])
}

fn vibration_effect(start: u8, strength: u8, frequency: u8) -> AdaptiveTriggerEffect {
    let mut zones = [0; 10];
    for zone in start.min(9)..=9 {
        zones[usize::from(zone)] = strength.clamp(1, 8);
    }

    let mut effect = feedback_effect(zones);
    effect.bytes[0] = 0x26;
    effect.bytes[9] = frequency;
    effect
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vibration_named_trigger_presets_use_persistent_vibration_mode() {
        let machine_gun = trigger_effect_for(AdaptiveTriggerPreset::MachineGun, 180);
        let pulse = trigger_effect_for(AdaptiveTriggerPreset::Pulse, 180);
        let bow = trigger_effect_for(AdaptiveTriggerPreset::Bow, 180);

        assert_eq!(machine_gun.bytes[0], 0x26);
        assert_eq!(pulse.bytes[0], 0x26);
        assert_eq!(bow.bytes[0], 0x21);
        assert_ne!(machine_gun.bytes[9], 0);
        assert_ne!(pulse.bytes[9], 0);
        assert_ne!(&machine_gun.bytes[1..7], &[0, 0, 0, 0, 0, 0]);
        assert_ne!(&pulse.bytes[1..7], &[0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn vibration_effect_uses_zone_mask_strength_pack_and_frequency() {
        let effect = vibration_effect(8, 8, 38);

        assert_eq!(effect.bytes[0], 0x26);
        assert_eq!(effect.bytes[1], 0x00);
        assert_eq!(effect.bytes[2], 0x03);
        assert_eq!(effect.bytes[6], 0x3f);
        assert_eq!(effect.bytes[9], 38);
    }

    #[test]
    fn pistol_trigger_uses_weapon_zone_mask_and_zero_based_strength() {
        let effect = trigger_effect_for(AdaptiveTriggerPreset::Pistol, 255);

        assert_eq!(effect.bytes, [0x25, 0x24, 0x00, 0x07, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn every_haptic_demo_contains_only_symmetric_samples() {
        for demo in HapticDemo::ALL {
            for frame in demo_frames(demo) {
                assert!(
                    frame.is_symmetric(),
                    "{} contains an asymmetric sample: {frame:?}",
                    demo.label()
                );
            }
        }
    }

    #[test]
    fn pulse_train_alternates_strength_without_alternating_motors() {
        let frames = demo_frames(HapticDemo::PulseTrain);
        let strengths: Vec<_> = frames.iter().map(|frame| frame.left).collect();

        assert_eq!(strengths, [220, 0, 140, 0, 220, 0, 140, 0]);
        assert_eq!(frames.last(), Some(&HapticFrame::symmetric(0, 70)));
    }

    #[test]
    fn impact_and_tap_demos_drive_both_motors_at_the_same_strength() {
        for demo in [HapticDemo::Impact, HapticDemo::Tap] {
            let frames = demo_frames(demo);
            let first = frames[0];

            assert!(first.left > 0, "{} should vibrate", demo.label());
            assert_eq!(first.left, first.right, "{} must be paired", demo.label());
            assert_eq!(frames.last(), Some(&HapticFrame::symmetric(0, 70)));
        }
    }

    #[test]
    fn weapon_effect_clamps_to_valid_weapon_positions() {
        let effect = weapon_effect(1, 3, 8);

        assert_eq!(effect.bytes, [0x25, 0x0c, 0x00, 0x07, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn custom_trigger_profiles_use_the_selected_protocol_mode() {
        let resistance = AdaptiveTriggerProfile {
            mode: AdaptiveTriggerMode::Resistance,
            start_position: 3,
            end_position: 6,
            intensity: 255,
            ..AdaptiveTriggerProfile::default()
        };
        let vibration = AdaptiveTriggerProfile {
            mode: AdaptiveTriggerMode::Vibration,
            start_position: 4,
            frequency: 73,
            intensity: 255,
            ..AdaptiveTriggerProfile::default()
        };

        let resistance_effect = trigger_effect_for_profile(&resistance);
        let vibration_effect = trigger_effect_for_profile(&vibration);

        assert_eq!(resistance_effect.bytes[0], 0x21);
        assert_eq!(resistance_effect.bytes[1], 0b0111_1000);
        assert_eq!(vibration_effect.bytes[0], 0x26);
        assert_eq!(vibration_effect.bytes[1], 0b1111_0000);
        assert_eq!(vibration_effect.bytes[2], 0b0000_0011);
        assert_eq!(vibration_effect.bytes[9], 73);
    }
}
