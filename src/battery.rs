use std::collections::HashMap;

use crate::catalog::{BatteryBaseline, DEFAULT_OVER_EAR_BASELINE};
use crate::models::{BatteryProjection, BatterySample, HeadphoneAge};

const EWMA_ALPHA: f64 = 0.25;
const MIN_STEP_MINUTES: f64 = 12.0;
const MAX_CONTINUOUS_GAP_MS: i64 = 6 * 60 * 60 * 1000;
const CALIBRATION_TARGET_TRANSITIONS: i32 = 3;

pub fn baseline_drain_per_hour(
    nc_active: bool,
    high_power_dsp: bool,
    age: HeadphoneAge,
    baseline: BatteryBaseline,
) -> f64 {
    let base_new_cell = match (nc_active, high_power_dsp) {
        (true, true) => baseline.high_dsp_anc_drain_per_hour(),
        (true, false) => baseline.anc_drain_per_hour(),
        (false, true) => baseline.high_dsp_off_drain_per_hour(),
        (false, false) => baseline.off_drain_per_hour(),
    };
    base_new_cell / age.capacity_multiplier()
}

pub fn project_battery(
    current_level: Option<i32>,
    is_charging: bool,
    nc_active: bool,
    high_power_dsp: bool,
    age: HeadphoneAge,
    samples: &[BatterySample],
    now_ms: i64,
    baseline: Option<BatteryBaseline>,
) -> BatteryProjection {
    let baseline = baseline.unwrap_or(DEFAULT_OVER_EAR_BASELINE);
    let mut buckets: HashMap<(bool, bool), f64> = HashMap::new();
    let mut valid_transitions: i32 = 0;
    let max_sane_drain =
        14.0_f64.max((baseline.high_dsp_anc_drain_per_hour() / age.capacity_multiplier()) * 2.5);

    if samples.len() >= 2 {
        let mut anchor = &samples[0];
        for i in 1..samples.len() {
            let curr = &samples[i];
            let gap_ms = curr.timestamp_ms - samples[i - 1].timestamp_ms;

            if curr.is_charging
                || anchor.is_charging
                || curr.level > anchor.level
                || gap_ms > MAX_CONTINUOUS_GAP_MS
                || gap_ms < 0
            {
                anchor = curr;
                continue;
            }

            if curr.level < anchor.level {
                let elapsed_minutes = (curr.timestamp_ms - anchor.timestamp_ms) as f64 / 60_000.0;
                if elapsed_minutes >= MIN_STEP_MINUTES {
                    let elapsed_hours = elapsed_minutes / 60.0;
                    let rate = (anchor.level - curr.level) as f64 / elapsed_hours;
                    if (0.5..=max_sane_drain).contains(&rate) {
                        let key = (anchor.nc_active, anchor.high_power_dsp);
                        let prior = *buckets.get(&key).unwrap_or(&baseline_drain_per_hour(
                            anchor.nc_active,
                            anchor.high_power_dsp,
                            age,
                            baseline,
                        ));
                        buckets.insert(key, EWMA_ALPHA * rate + (1.0 - EWMA_ALPHA) * prior);
                        valid_transitions += 1;
                    }
                }
                anchor = curr;
            }
        }
    }

    let active_key = (nc_active, high_power_dsp);
    let active_drain = *buckets
        .get(&active_key)
        .unwrap_or(&baseline_drain_per_hour(nc_active, high_power_dsp, age, baseline));
    let is_calibrated = valid_transitions >= CALIBRATION_TARGET_TRANSITIONS;
    let progress = (valid_transitions as f32 / CALIBRATION_TARGET_TRANSITIONS as f32).clamp(0.0, 1.0);

    let Some(level) = current_level else {
        return BatteryProjection {
            remaining_minutes: None,
            estimated_empty_at_ms: None,
            drain_rate_per_hour: active_drain,
            is_calibrated,
            valid_transitions,
            calibration_progress: progress,
        };
    };

    if is_charging || level <= 0 {
        return BatteryProjection {
            remaining_minutes: None,
            estimated_empty_at_ms: None,
            drain_rate_per_hour: active_drain,
            is_calibrated,
            valid_transitions,
            calibration_progress: progress,
        };
    }

    let remaining_hours = level as f64 / active_drain;
    let remaining_minutes = ((remaining_hours * 60.0) as i32).max(1);
    let empty_at = now_ms + (remaining_minutes as i64) * 60_000;

    BatteryProjection {
        remaining_minutes: Some(remaining_minutes),
        estimated_empty_at_ms: Some(empty_at),
        drain_rate_per_hour: active_drain,
        is_calibrated,
        valid_transitions,
        calibration_progress: progress,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baseline_projection_for_xm5() {
        let proj = project_battery(
            Some(90),
            false,
            true,
            false,
            HeadphoneAge::NewUnder1Y,
            &[],
            1_000_000,
            None,
        );
        // 30h total => 3.333%/hr => 90% = 27h = 1620m
        assert!(!proj.is_calibrated);
        assert_eq!(proj.remaining_minutes, Some(1620));
    }

    #[test]
    fn ewma_calibration_after_three_transitions() {
        let samples = vec![
            BatterySample {
                timestamp_ms: 0,
                level: 80,
                is_charging: false,
                nc_active: true,
                high_power_dsp: false,
                device_address: "AA:BB".into(),
            },
            BatterySample {
                timestamp_ms: 30 * 60_000,
                level: 78,
                is_charging: false,
                nc_active: true,
                high_power_dsp: false,
                device_address: "AA:BB".into(),
            },
            BatterySample {
                timestamp_ms: 60 * 60_000,
                level: 76,
                is_charging: false,
                nc_active: true,
                high_power_dsp: false,
                device_address: "AA:BB".into(),
            },
            BatterySample {
                timestamp_ms: 90 * 60_000,
                level: 74,
                is_charging: false,
                nc_active: true,
                high_power_dsp: false,
                device_address: "AA:BB".into(),
            },
        ];
        let proj = project_battery(
            Some(74),
            false,
            true,
            false,
            HeadphoneAge::NewUnder1Y,
            &samples,
            90 * 60_000,
            None,
        );
        assert!(proj.is_calibrated);
        assert_eq!(proj.valid_transitions, 3);
        assert!(proj.drain_rate_per_hour > 3.33);
    }
}
