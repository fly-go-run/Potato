//! Separate formula evaluation from XLSX serialization, as XlsxWriter does.
//! IronCalc supplies real cached results; no placeholder zeros are published.
use super::*;
use ironcalc_base::{cell::CellValue, Model};
use rust_xlsxwriter::{Format, Formula};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FormulaCell {
    formula: String,
}

fn formula(value: &Value) -> Result<Option<String>> {
    if !value.is_object() {
        return Ok(None);
    }
    let cell: FormulaCell = serde_json::from_value(value.clone())?;
    Ok(Some(cell.formula))
}

// A deliberately bounded numeric subset. Reject whole-column ranges, external
// references, volatile functions and enormous allocations before evaluation.
fn validate_formula(value: &str, rows: usize, columns: usize) -> Result<()> {
    if !value.starts_with('=') || value.len() > 512 {
        return Err(invalid(
            "Formula must begin with = and contain at most 512 bytes",
        ));
    }
    let mut depth = 0i32;
    let mut pos = 1;
    let bytes = value.as_bytes();
    while pos < bytes.len() {
        match bytes[pos] {
            b'(' => {
                depth += 1;
                if depth > 32 {
                    return Err(invalid("Formula nesting exceeds 32"));
                }
                pos += 1;
            }
            b')' => {
                depth -= 1;
                if depth < 0 {
                    return Err(invalid("Unbalanced formula"));
                }
                pos += 1;
            }
            b'A'..=b'Z' | b'$' => {
                let start = pos;
                while pos < bytes.len()
                    && (bytes[pos].is_ascii_alphanumeric() || bytes[pos] == b'$')
                {
                    pos += 1;
                }
                let token = value[start..pos].replace('$', "");
                if token.bytes().all(|b| b.is_ascii_uppercase()) {
                    if !matches!(
                        token.as_str(),
                        "SUM" | "AVERAGE" | "MIN" | "MAX" | "COUNT" | "ABS" | "ROUND"
                    ) {
                        return Err(invalid(
                            "Supported functions: SUM, AVERAGE, MIN, MAX, COUNT, ABS, ROUND",
                        ));
                    }
                } else {
                    let split = token
                        .find(|c: char| c.is_ascii_digit())
                        .ok_or_else(|| invalid("Invalid cell reference"))?;
                    let col = token[..split]
                        .bytes()
                        .try_fold(0usize, |n, b| {
                            n.checked_mul(26)?
                                .checked_add((b.checked_sub(b'A')? + 1) as usize)
                        })
                        .ok_or_else(|| invalid("Invalid column"))?;
                    let row = token[split..].parse::<usize>().map_err(invalid)?;
                    if col == 0 || col > columns || row == 0 || row > rows {
                        return Err(invalid(
                            "Formula references must stay inside the supplied sheet rectangle",
                        ));
                    }
                }
            }
            b'0'..=b'9' | b'.' | b'+' | b'-' | b'*' | b'/' | b',' | b':' | b' ' | b'%' => pos += 1,
            _ => {
                return Err(invalid(
                    "Use uppercase same-sheet numeric formulas; unsupported formula syntax",
                ))
            }
        }
    }
    if depth != 0 {
        return Err(invalid("Unbalanced formula"));
    }
    Ok(())
}

pub(super) fn generate(input: Workbook) -> Result<Vec<u8>> {
    bounded(input.sheets.len(), 20, "Sheets")?;
    let mut workbook = rust_xlsxwriter::Workbook::new();
    let mut total = 0;
    let mut formula_count = 0;
    for sheet in input.sheets {
        text(&sheet.name)?;
        bounded(sheet.rows.len(), 10000, "Rows")?;
        let columns = sheet.rows.iter().map(Vec::len).max().unwrap_or(0);
        bounded(columns, 1000, "Columns")?;
        total += sheet.rows.len() * columns;
        if total > 10000 {
            return Err(invalid("At most 10000 cells across sheet rectangles"));
        }
        if sheet.column_widths.len() > columns {
            return Err(invalid("Too many column widths"));
        }
        let mut model = Model::new_empty("calculation", "en", "UTC", "en").map_err(invalid)?;
        for (r, row) in sheet.rows.iter().enumerate() {
            for (c, cell) in row.iter().enumerate() {
                let value = if let Some(f) = formula(cell)? {
                    formula_count += 1;
                    if formula_count > 128 {
                        return Err(invalid("At most 128 formulas per workbook"));
                    }
                    validate_formula(&f, sheet.rows.len(), columns)?;
                    f
                } else {
                    match cell {
                        Value::Null => continue,
                        Value::String(s) => {
                            text(s)?;
                            format!("'{s}")
                        }
                        Value::Bool(b) => if *b { "TRUE" } else { "FALSE" }.to_owned(),
                        Value::Number(n) => n.to_string(),
                        _ => return Err(invalid("Invalid spreadsheet cell")),
                    }
                };
                model
                    .set_user_input(0, r as i32 + 1, c as i32 + 1, value)
                    .map_err(invalid)?;
            }
        }
        model.evaluate();
        let worksheet = workbook.add_worksheet();
        worksheet.set_name(&sheet.name).map_err(invalid)?;
        let header = Format::new()
            .set_bold()
            .set_background_color("17365D")
            .set_font_color("FFFFFF")
            .set_text_wrap();
        let mut normal = Format::new();
        if let Some(number_format) = sheet.number_format {
            text(&number_format)?;
            normal = normal.set_num_format(&number_format);
        }
        if sheet.header {
            worksheet.set_freeze_panes(1, 0).map_err(invalid)?;
            worksheet.set_row_height(0, 30).map_err(invalid)?;
        }
        for (c, width) in sheet.column_widths.into_iter().enumerate() {
            if !width.is_finite() || !(4.0..=80.0).contains(&width) {
                return Err(invalid("Column width must be 4..80"));
            }
            worksheet
                .set_column_width(c as u16, width)
                .map_err(invalid)?;
        }
        for (r, row) in sheet.rows.iter().enumerate() {
            for (c, cell) in row.iter().enumerate() {
                let format = if sheet.header && r == 0 {
                    &header
                } else {
                    &normal
                };
                if let Some(f) = formula(cell)? {
                    let result = model
                        .get_cell_value_by_index(0, r as i32 + 1, c as i32 + 1)
                        .map_err(invalid)?;
                    let CellValue::Number(value) = result else {
                        return Err(invalid(format!(
                            "Formula at {}!R{}C{} returned {result:?}; output was not saved",
                            sheet.name,
                            r + 1,
                            c + 1
                        )));
                    };
                    if !value.is_finite() {
                        return Err(invalid("Non-finite formula result"));
                    }
                    worksheet
                        .write_formula_with_format(
                            r as u32,
                            c as u16,
                            Formula::new(f).set_result(value.to_string()),
                            format,
                        )
                        .map_err(invalid)?;
                } else {
                    match cell {
                        Value::Null => {}
                        Value::String(s) => {
                            worksheet
                                .write_string_with_format(r as u32, c as u16, s, format)
                                .map_err(invalid)?;
                        }
                        Value::Bool(b) => {
                            worksheet
                                .write_boolean_with_format(r as u32, c as u16, *b, format)
                                .map_err(invalid)?;
                        }
                        Value::Number(n) => {
                            worksheet
                                .write_number_with_format(
                                    r as u32,
                                    c as u16,
                                    n.as_f64()
                                        .filter(|v| v.is_finite())
                                        .ok_or_else(|| invalid("Invalid number"))?,
                                    format,
                                )
                                .map_err(invalid)?;
                        }
                        _ => unreachable!(),
                    }
                }
            }
        }
    }
    workbook.save_to_buffer().map_err(invalid)
}
