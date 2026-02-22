//! Time series forecasting using augurs (MSTL + ETS).
//!
//! This module provides cost forecasting capabilities using proper time series
//! forecasting techniques rather than simple averages.
//!
//! # Algorithm
//!
//! - For data with >= 14 days: Uses MSTL (Multiple Seasonal-Trend decomposition)
//!   with weekly seasonality (period=7) combined with AutoETS for trend forecasting.
//!   This captures weekly patterns (weekday vs weekend usage differences).
//!
//! - For data with < 14 days: Falls back to simple AutoETS without seasonal decomposition.
//!
//! # Prediction Intervals
//!
//! All forecasts include 95% prediction intervals (configurable) to express uncertainty.
//! Wider intervals indicate higher uncertainty in the forecast.

use augurs::{
    ets::AutoETS,
    forecaster::{Forecaster, transforms::LinearInterpolator},
    mstl::MSTLModel,
};
use chrono::{Duration, NaiveDate};

use crate::models::{DailySpend, ForecastTimeSeries};

/// Minimum number of data points required for any forecasting
const MIN_DATA_POINTS: usize = 7;

/// Minimum data points for seasonal (MSTL) forecasting (need at least 2 full weeks)
const MIN_SEASONAL_DATA_POINTS: usize = 14;

/// Weekly seasonality period (7 days)
const WEEKLY_PERIOD: usize = 7;

/// Default forecast horizon (days)
pub const DEFAULT_FORECAST_DAYS: usize = 7;

/// Default confidence level for prediction intervals
const DEFAULT_CONFIDENCE_LEVEL: f64 = 0.95;

/// Error type for forecasting operations
#[derive(Debug, thiserror::Error)]
pub enum ForecastError {
    #[error("Insufficient data: need at least {MIN_DATA_POINTS} days, got {0}")]
    InsufficientData(usize),

    #[error("Forecast model error: {0}")]
    ModelError(String),

    #[error("Invalid forecast horizon: {0}")]
    InvalidHorizon(String),
}

/// Generate a time series forecast from daily spend data.
///
/// # Arguments
///
/// * `daily_spend` - Historical daily spend data (should be sorted by date)
/// * `forecast_days` - Number of days to forecast ahead
/// * `confidence_level` - Confidence level for prediction intervals (default: 0.95)
///
/// # Returns
///
/// A `ForecastTimeSeries` containing point forecasts and prediction intervals,
/// or `None` if there's insufficient data for forecasting.
pub fn generate_forecast(
    daily_spend: &[DailySpend],
    forecast_days: usize,
    confidence_level: Option<f64>,
) -> Result<Option<ForecastTimeSeries>, ForecastError> {
    if daily_spend.len() < MIN_DATA_POINTS {
        return Ok(None);
    }

    if forecast_days == 0 {
        return Err(ForecastError::InvalidHorizon(
            "forecast_days must be > 0".to_string(),
        ));
    }

    let confidence = confidence_level.unwrap_or(DEFAULT_CONFIDENCE_LEVEL);

    // Prepare time series data
    let (values, last_date) = prepare_time_series(daily_spend);

    // Choose forecasting strategy based on data length
    let use_seasonal = values.len() >= MIN_SEASONAL_DATA_POINTS;

    let forecast_result = if use_seasonal {
        forecast_with_mstl(&values, forecast_days, confidence)
    } else {
        forecast_with_ets(&values, forecast_days, confidence)
    };

    let forecast = forecast_result.map_err(|e| ForecastError::ModelError(e.to_string()))?;

    // Generate dates for forecast points
    let dates: Vec<NaiveDate> = (1..=forecast_days)
        .map(|i| last_date + Duration::days(i as i64))
        .collect();

    // Extract point forecasts and intervals
    let point_forecasts: Vec<f64> = forecast.point.iter().map(|v| v.max(0.0)).collect();

    let (lower_bounds, upper_bounds) = if let Some(ref intervals) = forecast.intervals {
        let lower: Vec<f64> = intervals.lower.iter().map(|v| v.max(0.0)).collect();
        let upper: Vec<f64> = intervals.upper.iter().map(|v| v.max(0.0)).collect();
        (lower, upper)
    } else {
        // Fallback: use point forecast +/- 20% if no intervals available
        let lower: Vec<f64> = point_forecasts.iter().map(|v| (v * 0.8).max(0.0)).collect();
        let upper: Vec<f64> = point_forecasts.iter().map(|v| v * 1.2).collect();
        (lower, upper)
    };

    Ok(Some(ForecastTimeSeries {
        dates,
        point_forecasts,
        lower_bounds,
        upper_bounds,
        confidence_level: confidence,
        used_seasonal_decomposition: use_seasonal,
    }))
}

/// Prepare time series data from daily spend records.
///
/// Fills in missing dates with zero spend and sorts by date.
/// Returns the values and the last date in the series.
fn prepare_time_series(daily_spend: &[DailySpend]) -> (Vec<f64>, NaiveDate) {
    if daily_spend.is_empty() {
        return (vec![], NaiveDate::from_ymd_opt(2000, 1, 1).unwrap());
    }

    // Sort by date
    let mut sorted: Vec<&DailySpend> = daily_spend.iter().collect();
    sorted.sort_by_key(|d| d.date);

    let first_date = sorted.first().unwrap().date;
    let last_date = sorted.last().unwrap().date;

    // Create a map for quick lookup
    let spend_map: std::collections::HashMap<NaiveDate, i64> = sorted
        .iter()
        .map(|d| (d.date, d.total_cost_microcents))
        .collect();

    // Fill in all dates (including missing ones with zero)
    let mut values = Vec::new();
    let mut current_date = first_date;
    while current_date <= last_date {
        let value = spend_map.get(&current_date).copied().unwrap_or(0);
        values.push(value as f64);
        current_date += Duration::days(1);
    }

    (values, last_date)
}

/// Forecast using MSTL (seasonal decomposition) with AutoETS trend model.
fn forecast_with_mstl(
    values: &[f64],
    horizon: usize,
    confidence: f64,
) -> Result<augurs::Forecast, Box<dyn std::error::Error + Send + Sync>> {
    // Use AutoETS as the trend model within MSTL
    let ets = AutoETS::non_seasonal().into_trend_model();

    // MSTL with weekly seasonality
    let mstl = MSTLModel::new(vec![WEEKLY_PERIOD], ets);

    // Use Forecaster with linear interpolation for any NaN values
    let transformers: Vec<Box<dyn augurs::forecaster::Transformer>> =
        vec![Box::new(LinearInterpolator::default())];
    let mut forecaster = Forecaster::new(mstl).with_transformers(transformers);

    forecaster
        .fit(values)
        .map_err(|e| format!("MSTL fit error: {e}"))?;

    forecaster
        .predict(horizon, confidence)
        .map_err(|e| format!("MSTL predict error: {e}").into())
}

/// Forecast using simple AutoETS (no seasonal decomposition).
fn forecast_with_ets(
    values: &[f64],
    horizon: usize,
    confidence: f64,
) -> Result<augurs::Forecast, Box<dyn std::error::Error + Send + Sync>> {
    // Non-seasonal ETS model
    let ets = AutoETS::non_seasonal();

    // Use Forecaster with linear interpolation for any NaN values
    let transformers: Vec<Box<dyn augurs::forecaster::Transformer>> =
        vec![Box::new(LinearInterpolator::default())];
    let mut forecaster = Forecaster::new(ets).with_transformers(transformers);

    forecaster
        .fit(values)
        .map_err(|e| format!("ETS fit error: {e}"))?;

    forecaster
        .predict(horizon, confidence)
        .map_err(|e| format!("ETS predict error: {e}").into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_daily_spend(date: NaiveDate, cost_microcents: i64) -> DailySpend {
        DailySpend {
            date,
            total_cost_microcents: cost_microcents,
            input_tokens: 0,
            output_tokens: 0,
            total_tokens: 0,
            request_count: 0,
            image_count: 0,
            audio_seconds: 0,
            character_count: 0,
        }
    }

    #[test]
    fn test_insufficient_data_returns_none() {
        let data: Vec<DailySpend> = (0..5)
            .map(|i| {
                make_daily_spend(
                    NaiveDate::from_ymd_opt(2025, 1, 1).unwrap() + Duration::days(i),
                    1000,
                )
            })
            .collect();

        let result = generate_forecast(&data, 7, None).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_simple_ets_forecast() {
        // 10 days of data - should use simple ETS (not MSTL)
        let data: Vec<DailySpend> = (0..10)
            .map(|i| {
                make_daily_spend(
                    NaiveDate::from_ymd_opt(2025, 1, 1).unwrap() + Duration::days(i),
                    100_000 + i * 1000, // Slight upward trend
                )
            })
            .collect();

        let result = generate_forecast(&data, 5, None).unwrap();
        assert!(result.is_some());

        let forecast = result.unwrap();
        assert_eq!(forecast.dates.len(), 5);
        assert_eq!(forecast.point_forecasts.len(), 5);
        assert_eq!(forecast.lower_bounds.len(), 5);
        assert_eq!(forecast.upper_bounds.len(), 5);
        assert!(!forecast.used_seasonal_decomposition);
        assert_eq!(forecast.confidence_level, 0.95);

        // Check dates are sequential
        assert_eq!(
            forecast.dates[0],
            NaiveDate::from_ymd_opt(2025, 1, 11).unwrap()
        );
    }

    #[test]
    fn test_mstl_forecast() {
        // 21 days of data - should use MSTL with weekly seasonality
        let data: Vec<DailySpend> = (0..21)
            .map(|i| {
                let weekday = i % 7;
                // Higher spend on weekdays (0-4), lower on weekends (5-6)
                let base = if weekday < 5 { 200_000 } else { 50_000 };
                make_daily_spend(
                    NaiveDate::from_ymd_opt(2025, 1, 1).unwrap() + Duration::days(i),
                    base + i * 500,
                )
            })
            .collect();

        let result = generate_forecast(&data, 7, None).unwrap();
        assert!(result.is_some());

        let forecast = result.unwrap();
        assert_eq!(forecast.dates.len(), 7);
        assert!(forecast.used_seasonal_decomposition);
    }

    #[test]
    fn test_missing_dates_filled() {
        // Data with gaps (only odd days)
        let data: Vec<DailySpend> = (0..15)
            .filter(|i| i % 2 == 1)
            .map(|i| {
                make_daily_spend(
                    NaiveDate::from_ymd_opt(2025, 1, 1).unwrap() + Duration::days(i),
                    100_000,
                )
            })
            .collect();

        let result = generate_forecast(&data, 3, None).unwrap();
        assert!(result.is_some());
    }

    #[test]
    fn test_custom_confidence_level() {
        let data: Vec<DailySpend> = (0..10)
            .map(|i| {
                make_daily_spend(
                    NaiveDate::from_ymd_opt(2025, 1, 1).unwrap() + Duration::days(i),
                    100_000,
                )
            })
            .collect();

        let result = generate_forecast(&data, 3, Some(0.90)).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().confidence_level, 0.90);
    }

    #[test]
    fn test_zero_forecast_days_error() {
        let data: Vec<DailySpend> = (0..10)
            .map(|i| {
                make_daily_spend(
                    NaiveDate::from_ymd_opt(2025, 1, 1).unwrap() + Duration::days(i),
                    100_000,
                )
            })
            .collect();

        let result = generate_forecast(&data, 0, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_forecasts_are_non_negative() {
        // Data that trends down sharply
        let data: Vec<DailySpend> = (0..10)
            .map(|i| {
                make_daily_spend(
                    NaiveDate::from_ymd_opt(2025, 1, 1).unwrap() + Duration::days(i),
                    (100_000 - i * 15_000).max(0), // Decreasing, might go negative
                )
            })
            .collect();

        let result = generate_forecast(&data, 7, None).unwrap();
        if let Some(forecast) = result {
            // All values should be >= 0
            for v in &forecast.point_forecasts {
                assert!(*v >= 0.0, "Point forecast should be non-negative: {v}");
            }
            for v in &forecast.lower_bounds {
                assert!(*v >= 0.0, "Lower bound should be non-negative: {v}");
            }
        }
    }
}
