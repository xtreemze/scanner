use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClockExchange {
    pub device_send_micros: u64,
    pub session_receive_micros: u64,
    pub session_send_micros: u64,
    pub device_receive_micros: u64,
}

impl ClockExchange {
    pub fn round_trip_micros(self) -> Option<u64> {
        let device_leg = self.device_receive_micros.checked_sub(self.device_send_micros)?;
        let session_leg = self.session_send_micros.checked_sub(self.session_receive_micros)?;
        device_leg.checked_sub(session_leg)
    }

    pub fn offset_micros(self) -> Option<f64> {
        if self.session_send_micros < self.session_receive_micros
            || self.device_receive_micros < self.device_send_micros
        {
            return None;
        }

        let t1 = self.device_send_micros as f64;
        let t2 = self.session_receive_micros as f64;
        let t3 = self.session_send_micros as f64;
        let t4 = self.device_receive_micros as f64;

        Some(((t2 - t1) + (t3 - t4)) * 0.5)
    }

    pub fn midpoint_device_micros(self) -> f64 {
        (self.device_send_micros as f64 + self.device_receive_micros as f64) * 0.5
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClockModel {
    pub reference_device_micros: f64,
    pub offset_micros: f64,
    pub drift_micros_per_micros: f64,
    pub uncertainty_micros: f64,
    pub sample_count: u32,
}

impl ClockModel {
    pub fn project_to_session_micros(self, device_micros: u64) -> Option<u64> {
        let local = device_micros as f64;
        let delta = local - self.reference_device_micros;
        let projected = local + self.offset_micros + self.drift_micros_per_micros * delta;
        if !projected.is_finite() || projected < 0.0 || projected > u64::MAX as f64 {
            return None;
        }
        Some(projected.round() as u64)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClockSyncError {
    InvalidExchange,
    TooFewSamples,
    DegenerateSamples,
}

#[derive(Debug, Default)]
pub struct DeviceClockSynchronizer {
    exchanges: BTreeMap<u64, ClockExchange>,
}

impl DeviceClockSynchronizer {
    pub fn add_exchange(&mut self, exchange: ClockExchange) -> Result<(), ClockSyncError> {
        let rtt = exchange
            .round_trip_micros()
            .ok_or(ClockSyncError::InvalidExchange)?;
        let offset = exchange
            .offset_micros()
            .ok_or(ClockSyncError::InvalidExchange)?;

        if !offset.is_finite() {
            return Err(ClockSyncError::InvalidExchange);
        }

        self.exchanges
            .insert(exchange.device_receive_micros.saturating_add(rtt), exchange);
        Ok(())
    }

    pub fn sample_count(&self) -> usize {
        self.exchanges.len()
    }

    pub fn model(&self) -> Result<ClockModel, ClockSyncError> {
        if self.exchanges.len() < 2 {
            return Err(ClockSyncError::TooFewSamples);
        }

        let samples = self
            .exchanges
            .values()
            .filter_map(|exchange| {
                let offset = exchange.offset_micros()?;
                let rtt = exchange.round_trip_micros()? as f64;
                let weight = 1.0 / (1.0 + rtt.max(0.0));
                Some((exchange.midpoint_device_micros(), offset, weight, rtt))
            })
            .collect::<Vec<_>>();

        if samples.len() < 2 {
            return Err(ClockSyncError::TooFewSamples);
        }

        let weight_sum = samples.iter().map(|(_, _, w, _)| *w).sum::<f64>();
        if !weight_sum.is_finite() || weight_sum <= 0.0 {
            return Err(ClockSyncError::DegenerateSamples);
        }

        let reference = samples
            .iter()
            .map(|(x, _, w, _)| x * w)
            .sum::<f64>()
            / weight_sum;
        let mean_offset = samples
            .iter()
            .map(|(_, y, w, _)| y * w)
            .sum::<f64>()
            / weight_sum;

        let variance_x = samples
            .iter()
            .map(|(x, _, w, _)| w * (x - reference).powi(2))
            .sum::<f64>();
        if !variance_x.is_finite() || variance_x <= 1e-9 {
            return Err(ClockSyncError::DegenerateSamples);
        }

        let covariance = samples
            .iter()
            .map(|(x, y, w, _)| w * (x - reference) * (y - mean_offset))
            .sum::<f64>();
        let drift = covariance / variance_x;

        let residual_variance = samples
            .iter()
            .map(|(x, y, w, _)| {
                let predicted = mean_offset + drift * (x - reference);
                w * (y - predicted).powi(2)
            })
            .sum::<f64>()
            / weight_sum;

        let half_best_rtt = samples
            .iter()
            .map(|(_, _, _, rtt)| *rtt)
            .fold(f64::INFINITY, f64::min)
            * 0.5;

        Ok(ClockModel {
            reference_device_micros: reference,
            offset_micros: mean_offset,
            drift_micros_per_micros: drift,
            uncertainty_micros: residual_variance.max(0.0).sqrt() + half_best_rtt,
            sample_count: samples.len() as u32,
        })
    }
}

#[derive(Debug, Default)]
pub struct SessionClockSynchronizer {
    devices: BTreeMap<String, DeviceClockSynchronizer>,
}

impl SessionClockSynchronizer {
    pub fn add_exchange(
        &mut self,
        device_id: impl Into<String>,
        exchange: ClockExchange,
    ) -> Result<(), ClockSyncError> {
        self.devices
            .entry(device_id.into())
            .or_default()
            .add_exchange(exchange)
    }

    pub fn model(&self, device_id: &str) -> Result<ClockModel, ClockSyncError> {
        self.devices
            .get(device_id)
            .ok_or(ClockSyncError::TooFewSamples)?
            .model()
    }

    pub fn project_to_session_micros(
        &self,
        device_id: &str,
        device_micros: u64,
    ) -> Result<u64, ClockSyncError> {
        self.model(device_id)?
            .project_to_session_micros(device_micros)
            .ok_or(ClockSyncError::DegenerateSamples)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exchange(device_send: u64, offset: i64, network_out: u64, network_back: u64) -> ClockExchange {
        let session_receive = (device_send as i128 + offset as i128 + network_out as i128) as u64;
        let session_send = session_receive + 100;
        let device_receive =
            (session_send as i128 - offset as i128 + network_back as i128) as u64;

        ClockExchange {
            device_send_micros: device_send,
            session_receive_micros: session_receive,
            session_send_micros: session_send,
            device_receive_micros: device_receive,
        }
    }

    #[test]
    fn estimates_constant_offset_and_uncertainty() {
        let mut sync = DeviceClockSynchronizer::default();
        sync.add_exchange(exchange(1_000_000, 2_500, 200, 200)).unwrap();
        sync.add_exchange(exchange(2_000_000, 2_500, 180, 220)).unwrap();
        sync.add_exchange(exchange(3_000_000, 2_500, 190, 210)).unwrap();

        let model = sync.model().unwrap();
        assert!((model.offset_micros - 2_500.0).abs() < 30.0);
        assert!(model.uncertainty_micros >= 190.0);
        assert_eq!(model.sample_count, 3);
    }

    #[test]
    fn estimates_linear_clock_drift() {
        let mut sync = DeviceClockSynchronizer::default();

        for i in 1..=5 {
            let local = i * 1_000_000;
            let offset = 1_000 + (i as i64 * 10);
            sync.add_exchange(exchange(local, offset, 100, 100)).unwrap();
        }

        let model = sync.model().unwrap();
        assert!((model.drift_micros_per_micros - 0.00001).abs() < 0.000002);

        let projected = model.project_to_session_micros(6_000_000).unwrap();
        assert!((projected as i64 - 6_001_060).abs() < 20);
    }

    #[test]
    fn lower_round_trip_samples_have_more_influence() {
        let mut sync = DeviceClockSynchronizer::default();
        sync.add_exchange(exchange(1_000_000, 2_000, 50, 50)).unwrap();
        sync.add_exchange(exchange(2_000_000, 2_000, 50, 50)).unwrap();
        sync.add_exchange(exchange(3_000_000, 5_000, 5_000, 5_000)).unwrap();

        let model = sync.model().unwrap();
        assert!(model.offset_micros < 2_100.0);
    }

    #[test]
    fn rejects_invalid_exchange_ordering() {
        let mut sync = DeviceClockSynchronizer::default();
        assert_eq!(
            sync.add_exchange(ClockExchange {
                device_send_micros: 100,
                session_receive_micros: 200,
                session_send_micros: 150,
                device_receive_micros: 250,
            }),
            Err(ClockSyncError::InvalidExchange)
        );
    }
}
