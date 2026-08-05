use serde_json::json;
use std::cmp::Ordering;

/// Calculate a spline for the given x and y data based on selected baseline points.
/// This implementation uses linear interpolation between the baseline points.
///
/// # Arguments
/// - `x`: Slice of x values.
/// - `y`: Slice of y values.
/// - `baseline_selected_points`: Slice of x-values chosen as baseline points.
/// - `interpolation_method`: Interpolation method (currently only "linear" is supported).
///
/// # Returns
/// A `Vec<f64>` containing the interpolated spline values for each x.
pub fn calculate_spline(
    x: &[f64],
    y: &[f64],
    baseline_selected_points: &[f64],
    interpolation_method: &str,
) -> Vec<f64> {
    // Build pairs (baseline point, corresponding y value); duplicate picks
    // at the same x would give the interpolation a zero-width segment
    let mut pairs: Vec<(f64, f64)> = baseline_selected_points
        .iter()
        .filter_map(|&bp| {
            x.iter()
                .position(|&xi| (xi - bp).abs() < 1e-6)
                .map(|i| (bp, y[i]))
        })
        .collect();
    pairs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal));
    pairs.dedup_by(|a, b| a.0 == b.0);

    if pairs.is_empty() {
        return vec![0.0; x.len()];
    }

    // For now we only support "linear" interpolation.
    assert!(
        (interpolation_method == "linear"),
        "Interpolation method {interpolation_method} not supported, only 'linear' is available"
    );

    let mut spline = Vec::with_capacity(x.len());
    for &xi in x {
        let yi = if xi <= pairs.first().unwrap().0 {
            pairs.first().unwrap().1
        } else if xi >= pairs.last().unwrap().0 {
            pairs.last().unwrap().1
        } else {
            // Find two consecutive pairs where xi fits.
            let mut interp = pairs.first().unwrap().1;
            for window in pairs.windows(2) {
                if xi >= window[0].0 && xi <= window[1].0 {
                    let (x0, y0) = window[0];
                    let (x1, y1) = window[1];
                    let t = (xi - x0) / (x1 - x0);
                    interp = y0 + t * (y1 - y0);
                    break;
                }
            }
            interp
        };
        spline.push(yi);
    }
    spline
}

/// Compute the filtered baseline by subtracting the spline from the original y values.
///
/// # Arguments
/// - `y`: Slice of original y values.
/// - `spline`: Slice of spline values (must be the same length as `y`).
///
/// # Returns
/// A `Vec<f64>` containing the baseline-filtered values.
pub fn filter_baseline(y: &[f64], spline: &[f64]) -> Vec<f64> {
    // zip stops at the shorter of the two, and everything downstream keys off
    // index against the time axis.
    if y.len() != spline.len() {
        tracing::error!(
            "Baseline spline has {} values against {} raw values; refusing to \
             produce a misaligned corrected signal",
            spline.len(),
            y.len()
        );
        return Vec::new();
    }
    y.iter().zip(spline.iter()).map(|(a, b)| a - b).collect()
}

/// Integrate the given data using the trapezoidal rule.
///
/// # Arguments
/// - `x`: Slice of x values.
/// - `y`: Slice of y values (must be the same length as `x`).
///
/// # Returns
/// The computed integral as an `f64`.
pub fn integrate_trapz(x: &[f64], y: &[f64]) -> f64 {
    let n = x.len();
    if n < 2 {
        return 0.0;
    }
    let mut area = 0.0;
    for i in 0..(n - 1) {
        let dx = x[i + 1] - x[i];
        let avg_y = f64::midpoint(y[i], y[i + 1]);
        area += dx * avg_y;
    }
    area
}

/// Integrate the given data using composite Simpson's rule with support for
/// unequally spaced samples, matching `scipy.integrate.simpson` (scipy >= 1.11).
///
/// When the number of intervals is odd, the trailing interval is handled with
/// scipy's Cartwright correction: a parabola through the last three points is
/// integrated over the last interval only.
///
/// Instrument exports can repeat a timestamp when the recorded time has fewer
/// significant digits than the sampling interval. A parabola is undefined over
/// such a zero-width interval, so those blocks fall back to the trapezoidal
/// rule, which keeps the total span and stays finite.
///
/// # Arguments
/// - `x`: Slice of x values.
/// - `y`: Slice of y values (must be the same length as `x`).
///
/// # Returns
/// The computed integral as an `f64`.
pub fn integrate_simpson(x: &[f64], y: &[f64]) -> f64 {
    let n = x.len();
    if n < 3 {
        return integrate_trapz(x, y);
    }
    let mut area = 0.0;
    let mut i = 0;
    while i + 2 < n {
        let h0 = x[i + 1] - x[i];
        let h1 = x[i + 2] - x[i + 1];
        if h0 <= 0.0 || h1 <= 0.0 {
            area += integrate_trapz(&x[i..=i + 2], &y[i..=i + 2]);
        } else {
            area += (h0 + h1) / 6.0
                * ((2.0 - h1 / h0) * y[i]
                    + ((h0 + h1).powi(2) / (h0 * h1)) * y[i + 1]
                    + (2.0 - h0 / h1) * y[i + 2]);
        }
        i += 2;
    }
    if (n - 1) % 2 == 1 {
        let h0 = x[n - 2] - x[n - 3];
        let h1 = x[n - 1] - x[n - 2];
        if h0 <= 0.0 || h1 <= 0.0 {
            area += integrate_trapz(&x[n - 2..=n - 1], &y[n - 2..=n - 1]);
        } else {
            let alpha = (2.0 * h1 * h1 + 3.0 * h0 * h1) / (6.0 * (h0 + h1));
            let beta = (h1 * h1 + 3.0 * h0 * h1) / (6.0 * h0);
            let eta = h1.powi(3) / (6.0 * h0 * (h0 + h1));
            area += alpha * y[n - 1] + beta * y[n - 2] - eta * y[n - 3];
        }
    }
    area
}

/// Converts coulombs to moles of electrons, matching `scipy.constants` in the
/// Python reference; stored prod `integral_results` are already mole values
/// (do not remove the division).
pub const FARADAY_C_PER_MOL: f64 = 96_485.332_12;

/// Calculate the integral for a given range using the specified integration method
/// and convert the area from coulombs to moles of electrons transferred, as in the
/// Python reference `integrate_coulomb_as_mole`.
///
/// # Arguments
/// - `x`: Slice of x values.
/// - `y`: Slice of y values.
/// - `integration_method`: A string specifying the method ("trapz" or "simpson").
///   Unknown methods fall back to "simpson" with a warning.
///
/// # Returns
/// The computed integral in moles as an `f64`.
pub fn calculate_integral_for_range(x: &[f64], y: &[f64], integration_method: &str) -> f64 {
    let area = match integration_method {
        "trapz" => integrate_trapz(x, y),
        "simpson" => integrate_simpson(x, y),
        other => {
            tracing::warn!("Integration method '{other}' not supported, using 'simpson'");
            integrate_simpson(x, y)
        }
    };
    area / FARADAY_C_PER_MOL
}

/// Null for a non-finite value, which JSON cannot carry.
fn finite_or_null(value: f64) -> serde_json::Value {
    if value.is_finite() {
        json!(value)
    } else {
        serde_json::Value::Null
    }
}

/// Calculate the integral for each pair in the provided list.
/// Each pair is expected to be a JSON object with the structure:
/// { "start": {"x": value}, "end": {"x": value}, "`sample_name"`: "..." }
///
/// # Arguments
/// - `pairs`: A slice of JSON values representing the pairs.
/// - `baseline_values`: Slice of baseline y values.
/// - `time_values`: Slice of time x values.
/// - `integration_method`: Integration method to use ("trapz" or "simpson").
///
/// # Returns
/// A vector of JSON objects, each containing "start", "end", "area", and "`sample_name`".
pub fn calculate_integrals_for_pairs(
    pairs: &[serde_json::Value],
    baseline_values: &[f64],
    time_values: &[f64],
    integration_method: &str,
) -> Vec<serde_json::Value> {
    let mut integration_results = Vec::new();

    // A mismatched baseline (e.g. cleared after ranges were chosen) cannot
    // be integrated against the time axis
    if baseline_values.len() != time_values.len() {
        return integration_results;
    }

    for pair in pairs {
        // Pairs still missing an endpoint are not integrable
        let Some(start) = pair
            .get("start")
            .and_then(|v| v.get("x"))
            .and_then(sea_orm::JsonValue::as_f64)
        else {
            continue;
        };
        let Some(end) = pair
            .get("end")
            .and_then(|v| v.get("x"))
            .and_then(sea_orm::JsonValue::as_f64)
        else {
            continue;
        };

        // Repeated timestamps are matched at their outer edges so the full
        // range is integrated regardless of which sample the click landed on.
        // Ordered first, so a range picked right-to-left covers the same
        // samples as the same range picked left-to-right.
        let (lower, upper) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        let start_index = time_values.iter().position(|&v| (v - lower).abs() < 1e-6);
        let end_index = time_values.iter().rposition(|&v| (v - upper).abs() < 1e-6);

        let sample_name = pair
            .get("sample_name")
            .and_then(|v| v.as_str())
            .unwrap_or("undefined")
            .to_string();

        // A range whose endpoints no longer land on a sample cannot be
        // integrated, and stays visible as unresolved rather than vanishing
        // while the plot goes on shading it.
        let (Some(si), Some(ei)) = (start_index, end_index) else {
            tracing::warn!(
                "Range '{sample_name}' spanning {lower}..{upper} does not match the \
                 channel's time values; reporting it as unresolved"
            );
            integration_results.push(json!({
                "start": start,
                "end": end,
                "area": serde_json::Value::Null,
                "sample_name": sample_name,
                "unresolved": true,
            }));
            continue;
        };

        {
            let (si, ei) = if si <= ei { (si, ei) } else { (ei, si) };
            let x_slice = &time_values[si..=ei];
            let y_slice = &baseline_values[si..=ei];
            let area = calculate_integral_for_range(x_slice, y_slice, integration_method);

            if !area.is_finite() {
                tracing::warn!(
                    "Integral for '{sample_name}' over {start}..{end} is not finite \
                     ({area}); reporting it as unavailable"
                );
            }

            integration_results.push(json!({
                "start": start,
                "end": end,
                "area": finite_or_null(area),
                "sample_name": sample_name,
            }));
        }
    }

    integration_results.sort_by(|a, b| {
        let a_start = a
            .get("start")
            .and_then(sea_orm::JsonValue::as_f64)
            .unwrap_or(0.0);
        let b_start = b
            .get("start")
            .and_then(sea_orm::JsonValue::as_f64)
            .unwrap_or(0.0);
        a_start.partial_cmp(&b_start).unwrap_or(Ordering::Equal)
    });

    integration_results
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine_series(points: u32) -> (Vec<f64>, Vec<f64>) {
        let x: Vec<f64> = (0..points).map(|i| f64::from(i) * 5.0).collect();
        let y: Vec<f64> = x.iter().map(|v| (v / 10.0).sin() + 2.0).collect();
        (x, y)
    }

    #[test]
    fn test_integrate_simpson_exact_for_quadratic() {
        let x: Vec<f64> = (0..=10).map(|i| f64::from(i) / 10.0).collect();
        let y: Vec<f64> = x.iter().map(|v| v * v).collect();
        assert!((integrate_simpson(&x, &y) - 1.0 / 3.0).abs() < 1e-12);
    }

    #[test]
    fn test_integrate_simpson_odd_interval_count() {
        // scipy.integrate.simpson(np.sin(x/10)+2, x=np.arange(0,50,5)) == 102.12931713299086
        let (x, y) = sine_series(10);
        assert!((integrate_simpson(&x, &y) - 102.129_317_132_990_86).abs() < 1e-9);
    }

    #[test]
    fn test_integrate_simpson_even_interval_count() {
        // scipy.integrate.simpson(np.sin(x/10)+2, x=np.arange(0,55,5)) == 107.16594144989648
        let (x, y) = sine_series(11);
        assert!((integrate_simpson(&x, &y) - 107.165_941_449_896_48).abs() < 1e-9);
    }

    #[test]
    fn test_integrate_trapz_reference() {
        // scipy.integrate.trapezoid(np.sin(x/10)+2, x=np.arange(0,55,5)) == 107.01351555505724
        let (x, y) = sine_series(11);
        assert!((integrate_trapz(&x, &y) - 107.013_515_555_057_24).abs() < 1e-9);
    }

    /// Timestamps printed with fewer digits than the sampling interval repeat,
    /// leaving zero-width intervals inside the integration range.
    fn repeated_timestamp_series() -> (Vec<f64>, Vec<f64>) {
        let mut x = vec![0.0];
        for i in 1..20 {
            x.push(f64::from(i / 2 * 10));
        }
        let y: Vec<f64> = (0..20).map(|i| (f64::from(i) / 4.0).sin() + 2.0).collect();
        (x, y)
    }

    #[test]
    fn test_integrate_simpson_handles_repeated_timestamps() {
        let (x, y) = repeated_timestamp_series();
        let simpson = integrate_simpson(&x, &y);
        assert!(simpson.is_finite(), "simpson returned {simpson}");
        assert!((simpson - integrate_trapz(&x, &y)).abs() / simpson.abs() < 0.05);
    }

    #[test]
    fn test_non_finite_area_serialises_as_null() {
        let time_values = vec![0.0, 10.0, 20.0];
        let baseline_values = vec![0.0, f64::NAN, 0.0];
        let pairs = vec![json!({"start": {"x": 0.0}, "end": {"x": 20.0}, "sample_name": "a"})];
        let results = calculate_integrals_for_pairs(&pairs, &baseline_values, &time_values, "trapz");

        assert_eq!(results.len(), 1);
        assert!(results[0].get("area").unwrap().is_null());
        // The row is still present, so a null area means "computed and failed"
        assert_eq!(results[0].get("sample_name").unwrap(), "a");
    }

    // A range is a span, not a direction
    #[test]
    fn test_reversed_range_matches_forward_range() {
        let (time_values, baseline_values) = repeated_timestamp_series();
        let forward = json!({"start": {"x": 0.0}, "end": {"x": 90.0}, "sample_name": "f"});
        let reversed = json!({"start": {"x": 90.0}, "end": {"x": 0.0}, "sample_name": "r"});

        let area_of = |pair: serde_json::Value| {
            calculate_integrals_for_pairs(&[pair], &baseline_values, &time_values, "trapz")[0]
                .get("area")
                .and_then(serde_json::Value::as_f64)
                .expect("area should be a number")
        };

        assert!((area_of(forward) - area_of(reversed)).abs() < 1e-12);
    }

    // Both ends resolve outwards, whichever duplicate the click landed on
    #[test]
    fn test_repeated_boundary_timestamps_resolve_to_outer_edges() {
        let time_values = vec![0.0, 10.0, 10.0, 20.0, 30.0, 30.0, 40.0];
        let baseline_values = vec![1.0; 7];
        let pairs = vec![json!({"start": {"x": 10.0}, "end": {"x": 30.0}, "sample_name": "a"})];

        let area = calculate_integrals_for_pairs(&pairs, &baseline_values, &time_values, "trapz")[0]
            .get("area")
            .and_then(serde_json::Value::as_f64)
            .unwrap();

        // 10..30 at unit height is 20 coulombs before the Faraday conversion
        assert!((area - 20.0 / FARADAY_C_PER_MOL).abs() < 1e-12);
    }

    #[test]
    fn test_zero_width_range_integrates_to_zero() {
        let time_values = vec![0.0, 10.0, 20.0];
        let baseline_values = vec![1.0, 2.0, 3.0];
        let pairs = vec![json!({"start": {"x": 10.0}, "end": {"x": 10.0}, "sample_name": "a"})];

        let area = calculate_integrals_for_pairs(&pairs, &baseline_values, &time_values, "trapz")[0]
            .get("area")
            .and_then(serde_json::Value::as_f64)
            .unwrap();

        assert!(area.abs() < 1e-12);
    }

    #[test]
    fn test_unmatched_range_is_reported_not_dropped() {
        let time_values = vec![0.0, 10.0, 20.0];
        let baseline_values = vec![1.0, 1.0, 1.0];
        let pairs = vec![
            json!({"start": {"x": 0.0}, "end": {"x": 20.0}, "sample_name": "good"}),
            json!({"start": {"x": 5.5}, "end": {"x": 17.5}, "sample_name": "stale"}),
        ];

        let results =
            calculate_integrals_for_pairs(&pairs, &baseline_values, &time_values, "trapz");

        assert_eq!(results.len(), 2, "the unmatched range should still appear");
        let stale = results
            .iter()
            .find(|r| r.get("sample_name").unwrap() == "stale")
            .unwrap();
        assert_eq!(stale.get("unresolved").unwrap(), true);
        assert!(stale.get("area").unwrap().is_null());
    }

    #[test]
    fn test_misaligned_baseline_yields_no_results() {
        let time_values = vec![0.0, 10.0, 20.0, 30.0];
        let baseline_values = vec![1.0, 1.0];
        let pairs = vec![json!({"start": {"x": 0.0}, "end": {"x": 30.0}, "sample_name": "a"})];

        let results =
            calculate_integrals_for_pairs(&pairs, &baseline_values, &time_values, "trapz");
        assert!(results.is_empty());
    }

    #[test]
    fn test_filter_baseline_refuses_misaligned_input() {
        assert_eq!(filter_baseline(&[1.0, 2.0, 3.0], &[0.5, 0.5, 0.5]).len(), 3);
        assert!(filter_baseline(&[1.0, 2.0, 3.0], &[0.5, 0.5]).is_empty());
    }

    #[test]
    fn test_no_pairs_yields_no_results() {
        let time_values = vec![0.0, 10.0];
        let baseline_values = vec![1.0, 1.0];
        assert!(calculate_integrals_for_pairs(&[], &baseline_values, &time_values, "trapz").is_empty());
    }

    // An open pair is not integrable, and not a failure either
    #[test]
    fn test_open_pair_is_skipped() {
        let time_values = vec![0.0, 10.0, 20.0];
        let baseline_values = vec![1.0, 1.0, 1.0];
        let pairs = vec![json!({"start": {"x": 0.0}, "sample_name": "open"})];

        let results =
            calculate_integrals_for_pairs(&pairs, &baseline_values, &time_values, "trapz");
        assert!(results.is_empty());
    }

    #[test]
    fn test_calculate_integrals_for_pairs_with_repeated_timestamps() {
        let (time_values, baseline_values) = repeated_timestamp_series();
        let pairs = vec![json!({"start": {"x": 0.0}, "end": {"x": 90.0}, "sample_name": "a"})];
        let results =
            calculate_integrals_for_pairs(&pairs, &baseline_values, &time_values, "simpson");
        let area = results[0]
            .get("area")
            .and_then(serde_json::Value::as_f64)
            .expect("area should serialise as a number, not null");
        assert!(area.is_finite());
    }

    #[test]
    fn test_calculate_integral_for_range_converts_to_moles() {
        let (x, y) = sine_series(11);
        let trapz_mol = calculate_integral_for_range(&x, &y, "trapz");
        let simpson_mol = calculate_integral_for_range(&x, &y, "simpson");
        let unknown_mol = calculate_integral_for_range(&x, &y, "not-a-method");
        assert!((trapz_mol - integrate_trapz(&x, &y) / FARADAY_C_PER_MOL).abs() < 1e-15);
        assert!((simpson_mol - integrate_simpson(&x, &y) / FARADAY_C_PER_MOL).abs() < 1e-15);
        assert!((unknown_mol - simpson_mol).abs() < 1e-15);
    }

    #[test]
    fn test_calculate_integrals_for_pairs_simpson() {
        let (time_values, baseline_values) = sine_series(11);
        let pairs = vec![
            json!({"start": {"x": 0.0}, "end": {"x": 20.0}, "sample_name": "a"}),
            json!({"start": {"x": 25.0}, "end": {"x": 50.0}, "sample_name": "b"}),
        ];
        let results =
            calculate_integrals_for_pairs(&pairs, &baseline_values, &time_values, "simpson");
        assert_eq!(results.len(), 2);
        let expected =
            calculate_integral_for_range(&time_values[0..=4], &baseline_values[0..=4], "simpson");
        let area = results[0]
            .get("area")
            .and_then(serde_json::Value::as_f64)
            .unwrap();
        assert!((area - expected).abs() < 1e-15);
        assert_eq!(results[1].get("sample_name").unwrap(), "b");
    }

    #[test]
    fn test_calculate_integrals_skips_incomplete_pairs() {
        let (time_values, baseline_values) = sine_series(11);
        let pairs = vec![
            json!({"start": {"x": 0.0}}),
            json!({"start": {"x": 5.0}, "end": {"x": 25.0}, "sample_name": "a"}),
        ];
        let results =
            calculate_integrals_for_pairs(&pairs, &baseline_values, &time_values, "trapz");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].get("sample_name").unwrap(), "a");
    }

    #[test]
    fn test_calculate_integrals_requires_matching_baseline() {
        let (time_values, _) = sine_series(11);
        let pairs = vec![json!({"start": {"x": 0.0}, "end": {"x": 25.0}, "sample_name": "a"})];
        let results = calculate_integrals_for_pairs(&pairs, &[], &time_values, "trapz");
        assert!(results.is_empty());
    }

    #[test]
    fn test_calculate_spline_ignores_duplicate_picks() {
        let (x, y) = sine_series(10);
        let spline = calculate_spline(&x, &y, &[0.0, 20.0, 20.0, 45.0], "linear");
        assert!(spline.iter().all(|v| v.is_finite()));
    }

    /// The workflow the API serves (pair selection, trapezoidal integration,
    /// conversion to moles) must reproduce the SOIL lab's MATLAB outputs on
    /// the lab's own data; the same fixtures back lab-codes
    /// tests/test_integration.py.
    #[test]
    fn test_served_integration_matches_matlab_reference() {
        let baseline_csv = include_str!("../fixtures/matlab_baseline_filtered.csv");
        let integral_csv = include_str!("../fixtures/matlab_integral_output.csv");

        let mut lines = baseline_csv.lines();
        let header: Vec<&str> = lines.next().unwrap().trim().split(',').collect();
        let rows: Vec<Vec<f64>> = lines
            .filter(|l| !l.trim().is_empty())
            .map(|l| {
                l.trim()
                    .split(',')
                    .map(|v| v.parse::<f64>().unwrap())
                    .collect()
            })
            .collect();
        let time: Vec<f64> = rows.iter().map(|r| r[0]).collect();

        let mut checked = 0;
        for line in integral_csv.lines().skip(1).filter(|l| !l.trim().is_empty()) {
            let fields: Vec<&str> = line.trim().split(',').collect();
            let measurement = fields[0];
            let start: f64 = fields[1].parse().unwrap();
            let end: f64 = fields[2].parse().unwrap();
            let reference_mol: f64 = fields[4].parse().unwrap();

            let col = header.iter().position(|h| *h == measurement).unwrap();
            let baseline: Vec<f64> = rows.iter().map(|r| r[col]).collect();
            let pairs = vec![json!({
                "start": {"x": start},
                "end": {"x": end},
                "sample_name": measurement,
            })];

            let results = calculate_integrals_for_pairs(&pairs, &baseline, &time, "trapz");
            assert_eq!(results.len(), 1, "no result for {measurement}");
            let area = results[0]
                .get("area")
                .and_then(serde_json::Value::as_f64)
                .unwrap();
            let rel_err = ((area - reference_mol) / reference_mol).abs();
            assert!(rel_err < 1e-3, "measurement {measurement}: rel err {rel_err}");
            checked += 1;
        }
        assert!(checked > 0, "no reference rows checked");
    }

    #[test]
    fn test_integrals_match_matlab_reference() {
        // Fixtures from the SOIL lab MATLAB workflow; the int column is in moles,
        // matching the Python reference test in lab-codes tests/test_integration.py.
        let baseline_csv = include_str!("../fixtures/matlab_baseline_filtered.csv");
        let integral_csv = include_str!("../fixtures/matlab_integral_output.csv");

        let mut lines = baseline_csv.lines();
        let header: Vec<&str> = lines.next().unwrap().trim().split(',').collect();
        let rows: Vec<Vec<f64>> = lines
            .filter(|l| !l.trim().is_empty())
            .map(|l| {
                l.trim()
                    .split(',')
                    .map(|v| v.parse::<f64>().unwrap())
                    .collect()
            })
            .collect();
        let time: Vec<f64> = rows.iter().map(|r| r[0]).collect();

        for line in integral_csv.lines().skip(1).filter(|l| !l.trim().is_empty()) {
            let fields: Vec<&str> = line.trim().split(',').collect();
            let measurement = fields[0];
            let start: f64 = fields[1].parse().unwrap();
            let end: f64 = fields[2].parse().unwrap();
            let reference_mol: f64 = fields[4].parse().unwrap();

            let col = header.iter().position(|h| *h == measurement).unwrap();
            let x: Vec<f64> = time
                .iter()
                .copied()
                .filter(|&t| t >= start && t <= end)
                .collect();
            let y: Vec<f64> = rows
                .iter()
                .filter(|r| r[0] >= start && r[0] <= end)
                .map(|r| r[col])
                .collect();

            let calc_mol = integrate_trapz(&x, &y) / FARADAY_C_PER_MOL;
            let rel_err = ((calc_mol - reference_mol) / reference_mol).abs();
            assert!(
                rel_err < 1e-3,
                "measurement {measurement}: rel err {rel_err}"
            );
        }
    }
}
