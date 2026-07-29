use chrono::{DateTime, NaiveDateTime, Utc};
use std::fmt;

#[derive(Debug)]
pub struct ParsedInstrumentFile {
    pub date: Option<DateTime<Utc>>,
    pub instrument_model: Option<String>,
    pub device_filename: Option<String>,
    pub data_source: Option<String>,
    pub init_e: Option<f64>,
    pub sample_interval: Option<f64>,
    pub run_time: Option<f64>,
    pub quiet_time: Option<f64>,
    pub sensitivity: Option<f64>,
    pub channel_names: Vec<String>,
    pub time_values: Vec<f64>,
    pub channel_values: Vec<Vec<f64>>,
}

#[derive(Debug)]
pub enum InstrumentParseError {
    InvalidBase64(String),
    NotUtf8,
    HeaderNotFound,
    NoDataRows,
    MalformedRow { line: usize, reason: String },
}

impl fmt::Display for InstrumentParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBase64(reason) => write!(f, "invalid base64 data: {reason}"),
            Self::NotUtf8 => write!(f, "file is not valid UTF-8 text"),
            Self::HeaderNotFound => {
                write!(f, "no data header (a line starting with 'Time/s') found")
            }
            Self::NoDataRows => write!(f, "no data rows found after the data header"),
            Self::MalformedRow { line, reason } => {
                write!(f, "malformed data row at line {line}: {reason}")
            }
        }
    }
}

impl std::error::Error for InstrumentParseError {}

#[derive(Default)]
struct Preamble {
    instrument_model: Option<String>,
    device_filename: Option<String>,
    data_source: Option<String>,
    init_e: Option<f64>,
    sample_interval: Option<f64>,
    run_time: Option<f64>,
    quiet_time: Option<f64>,
    sensitivity: Option<f64>,
}

/// Decodes a base64 data URL and parses it as a CHI instrument export.
///
/// # Errors
///
/// Returns an `InstrumentParseError` if the payload cannot be decoded,
/// is not UTF-8 text, or does not parse as an instrument file.
pub fn process_instrument_data_base64(
    data_base64: &str,
) -> Result<ParsedInstrumentFile, InstrumentParseError> {
    let (raw_data, _file_type) = crate::common::upload::decode_base64(data_base64)
        .map_err(InstrumentParseError::InvalidBase64)?;
    let text = std::str::from_utf8(&raw_data).map_err(|_| InstrumentParseError::NotUtf8)?;
    parse_instrument_file(text)
}

/// Parses the text of a CHI instrument export (tab or comma delimited).
///
/// # Errors
///
/// Returns an `InstrumentParseError` if the data header is missing, no data
/// rows follow it, or a data row is malformed.
pub fn parse_instrument_file(text: &str) -> Result<ParsedInstrumentFile, InstrumentParseError> {
    let lines: Vec<&str> = text.lines().map(str::trim_end).collect();
    let (header_idx, delimiter) =
        find_data_header(&lines).ok_or(InstrumentParseError::HeaderNotFound)?;

    let header_tokens: Vec<String> = lines[header_idx]
        .split(delimiter)
        .map(|token| token.trim().to_string())
        .collect();
    let channel_names: Vec<String> = header_tokens[1..].to_vec();
    if channel_names.is_empty() {
        return Err(InstrumentParseError::HeaderNotFound);
    }

    let (time_values, channel_values) =
        parse_data_rows(&lines, header_idx, delimiter, header_tokens.len())?;
    let preamble = parse_preamble(&lines[..header_idx]);
    let date = lines.first().and_then(|line| parse_date(line));

    Ok(ParsedInstrumentFile {
        date,
        instrument_model: preamble.instrument_model,
        device_filename: preamble.device_filename,
        data_source: preamble.data_source,
        init_e: preamble.init_e,
        sample_interval: preamble.sample_interval,
        run_time: preamble.run_time,
        quiet_time: preamble.quiet_time,
        sensitivity: preamble.sensitivity,
        channel_names,
        time_values,
        channel_values,
    })
}

fn find_data_header(lines: &[&str]) -> Option<(usize, char)> {
    lines.iter().enumerate().find_map(|(idx, line)| {
        if line.trim_start().starts_with("Time/s") {
            let delimiter = if line.contains('\t') { '\t' } else { ',' };
            Some((idx, delimiter))
        } else {
            None
        }
    })
}

fn parse_data_rows(
    lines: &[&str],
    header_idx: usize,
    delimiter: char,
    column_count: usize,
) -> Result<(Vec<f64>, Vec<Vec<f64>>), InstrumentParseError> {
    let mut time_values = Vec::new();
    let mut channel_values = vec![Vec::new(); column_count - 1];

    for (offset, line) in lines[header_idx + 1..].iter().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split(delimiter).map(str::trim).collect();
        if fields.len() < 2 {
            continue;
        }
        let file_line = header_idx + offset + 2;
        if fields.len() != column_count {
            return Err(InstrumentParseError::MalformedRow {
                line: file_line,
                reason: format!("expected {column_count} fields, found {}", fields.len()),
            });
        }
        let mut values = Vec::with_capacity(column_count);
        for field in &fields {
            let value = field
                .parse::<f64>()
                .map_err(|_| InstrumentParseError::MalformedRow {
                    line: file_line,
                    reason: format!("field '{field}' is not a number"),
                })?;
            values.push(value);
        }
        time_values.push(values[0]);
        for (channel, value) in channel_values.iter_mut().zip(&values[1..]) {
            channel.push(*value);
        }
    }

    if time_values.is_empty() {
        return Err(InstrumentParseError::NoDataRows);
    }
    Ok((time_values, channel_values))
}

fn parse_date(line: &str) -> Option<DateTime<Utc>> {
    let mut tokens: Vec<&str> = line.split_whitespace().collect();
    let month = tokens.first()?.trim_end_matches('.');
    tokens[0] = month;
    let normalised = tokens.join(" ");

    for format in ["%B %d, %Y %H:%M:%S", "%b %d, %Y %H:%M:%S"] {
        if let Ok(naive) = NaiveDateTime::parse_from_str(&normalised, format) {
            return Some(naive.and_utc());
        }
    }
    None
}

fn parse_preamble(lines: &[&str]) -> Preamble {
    let mut preamble = Preamble::default();
    for line in lines {
        let trimmed = line.trim();
        if preamble.device_filename.is_none() {
            preamble.device_filename = match_text(trimmed, "File:");
        }
        if preamble.data_source.is_none() {
            preamble.data_source = match_text(trimmed, "Data Source:");
        }
        if preamble.instrument_model.is_none() {
            preamble.instrument_model = match_text(trimmed, "Instrument Model:");
        }
        if preamble.init_e.is_none() {
            preamble.init_e = match_number(trimmed, "Init E (V)");
        }
        if preamble.sample_interval.is_none() {
            preamble.sample_interval = match_number(trimmed, "Sample Interval (s)");
        }
        if preamble.run_time.is_none() {
            preamble.run_time = match_number(trimmed, "Run Time (sec)");
        }
        if preamble.quiet_time.is_none() {
            preamble.quiet_time = match_number(trimmed, "Quiet Time (sec)");
        }
        if preamble.sensitivity.is_none() {
            preamble.sensitivity = match_number(trimmed, "Sensitivity (A/V)");
        }
    }
    preamble
}

fn match_text(line: &str, label: &str) -> Option<String> {
    if !line.starts_with(label) {
        return None;
    }
    let (_, rest) = line.split_once(':')?;
    let rest = rest.trim();
    if rest.is_empty() {
        None
    } else {
        Some(rest.to_string())
    }
}

fn match_number(line: &str, label: &str) -> Option<f64> {
    if !line.starts_with(label) {
        return None;
    }
    let (_, rest) = line.split_once('=')?;
    rest.trim().parse::<f64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    const CHI1000B_TAB: &str = include_str!("fixtures/chi1000b_tab.txt");
    const CHI1030C_COMMA: &str = include_str!("fixtures/chi1030c_comma.txt");
    const CHI1030C_6COL_COMMA: &str = include_str!("fixtures/chi1030c_6col_comma.txt");

    #[test]
    fn test_parse_instrument_file_tab_delimited() {
        let parsed = parse_instrument_file(CHI1000B_TAB).unwrap();

        assert_eq!(
            parsed.channel_names,
            vec!["i1/A", "i2/A", "i3/A", "i4/A", "i5/A", "i6/A", "i7/A", "i8/A"]
        );
        assert_eq!(parsed.time_values.len(), 20);
        assert!((parsed.time_values[0] - 5.0).abs() < f64::EPSILON);
        assert_eq!(parsed.channel_values.len(), 8);
        assert!((parsed.channel_values[0][0] - 4.626e-6).abs() < 1e-12);

        assert_eq!(
            parsed.date,
            Some(Utc.with_ymd_and_hms(2026, 7, 23, 16, 37, 16).unwrap())
        );
        assert_eq!(parsed.instrument_model.as_deref(), Some("CHI1000B"));
        assert_eq!(parsed.device_filename.as_deref(), Some("260723_samples100ul"));
        assert_eq!(parsed.data_source.as_deref(), Some("Experiment"));
        assert_eq!(parsed.init_e, Some(-0.78));
        assert_eq!(parsed.sample_interval, Some(5.0));
        assert_eq!(parsed.run_time, Some(2.5e4));
        assert_eq!(parsed.quiet_time, Some(0.0));
        assert_eq!(parsed.sensitivity, Some(1e-4));
    }

    #[test]
    fn test_parse_instrument_file_comma_delimited() {
        let parsed = parse_instrument_file(CHI1030C_COMMA).unwrap();

        assert_eq!(parsed.channel_names.len(), 8);
        assert_eq!(
            parsed.date,
            Some(Utc.with_ymd_and_hms(2023, 8, 17, 21, 18, 50).unwrap())
        );
        assert_eq!(parsed.instrument_model.as_deref(), Some("CHI1030C"));
        assert_eq!(
            parsed.device_filename.as_deref(),
            Some("230817_ls_manganite_004.bin")
        );
        // Sens2..Sens8 lines must not overwrite the first Sensitivity value
        assert_eq!(parsed.sensitivity, Some(1e-4));
    }

    #[test]
    fn test_parse_instrument_file_six_channel_comma() {
        let parsed = parse_instrument_file(CHI1030C_6COL_COMMA).unwrap();

        assert_eq!(
            parsed.channel_names,
            vec!["i1/A", "i2/A", "i3/A", "i5/A", "i6/A", "i7/A"]
        );
        assert_eq!(parsed.channel_values.len(), 6);
    }

    #[test]
    fn test_parse_instrument_file_missing_header() {
        let text = "some preamble\n1.0, 2.0\n";
        let err = parse_instrument_file(text).unwrap_err();
        assert!(matches!(err, InstrumentParseError::HeaderNotFound));
    }

    #[test]
    fn test_parse_instrument_file_malformed_row() {
        let text = "Time/s, i1/A\n5.0, 1.0\n10.0, abc\n";
        let err = parse_instrument_file(text).unwrap_err();
        match err {
            InstrumentParseError::MalformedRow { line, reason } => {
                assert_eq!(line, 3);
                assert!(reason.contains("abc"));
            }
            other => panic!("expected MalformedRow, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_instrument_file_short_final_row_skipped() {
        let text = "Time/s, i1/A\n5.0, 1.0\n10.0\n";
        let parsed = parse_instrument_file(text).unwrap();
        assert_eq!(parsed.time_values, vec![5.0]);
        assert_eq!(parsed.channel_values, vec![vec![1.0]]);
    }

    #[test]
    fn test_parse_instrument_file_unparsable_first_line() {
        let text = "not a date at all\nTime/s, i1/A\n5.0, 1.0\n";
        let parsed = parse_instrument_file(text).unwrap();
        assert_eq!(parsed.date, None);
        assert_eq!(parsed.time_values, vec![5.0]);
    }
}
