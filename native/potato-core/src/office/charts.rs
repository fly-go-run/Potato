//! One data source for chart caches and the editable embedded workbook.
use super::*;
use std::{
    collections::BTreeMap,
    io::{Read, Write},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ChartSpec {
    kind: String,
    title: String,
    categories: Vec<String>,
    series: Vec<Series>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Series {
    name: String,
    values: Vec<f64>,
}

impl ChartSpec {
    pub(super) fn validate(&self) -> Result<()> {
        if !matches!(self.kind.as_str(), "column" | "bar" | "line" | "pie") {
            return Err(invalid("Chart kind must be column, bar, line or pie"));
        }
        text(&self.title)?;
        bounded(self.categories.len(), 30, "Chart categories")?;
        bounded(self.series.len(), 6, "Chart series")?;
        if self.kind == "pie" && self.series.len() != 1 {
            return Err(invalid("Pie charts require one series"));
        }
        for category in &self.categories {
            text(category)?;
        }
        for series in &self.series {
            text(&series.name)?;
            if series.values.len() != self.categories.len()
                || series.values.iter().any(|v| !v.is_finite())
            {
                return Err(invalid("Chart values must be finite and match categories"));
            }
        }
        Ok(())
    }
    pub(super) fn placeholder(&self) -> ppt_rs::generator::charts::Chart {
        use ppt_rs::generator::charts::{ChartBuilder, ChartSeries, ChartType};
        // Fixed rectangle under the title on a 10 x 7.5 inch slide. No overlap
        // with body text is possible because chart slides have no bullets.
        ChartBuilder::new(&self.title, ChartType::Bar)
            .categories(self.categories.iter().map(String::as_str).collect())
            .add_series(ChartSeries::new(
                &self.series[0].name,
                self.series[0].values.clone(),
            ))
            .position(457200, 1463040)
            .size(8230200, 4937760)
            .build()
    }
    pub(super) fn package(&self) -> Result<(Vec<u8>, Vec<u8>)> {
        use rust_xlsxwriter::{Chart, ChartType};
        let mut workbook = rust_xlsxwriter::Workbook::new();
        let sheet = workbook.add_worksheet();
        sheet.set_name("Sheet1").map_err(invalid)?;
        for (r, label) in self.categories.iter().enumerate() {
            sheet
                .write_string(r as u32 + 1, 0, label)
                .map_err(invalid)?;
        }
        let kind = match self.kind.as_str() {
            "bar" => ChartType::Bar,
            "line" => ChartType::Line,
            "pie" => ChartType::Pie,
            _ => ChartType::Column,
        };
        let mut chart = Chart::new(kind);
        chart.title().set_name(&self.title);
        chart.set_style(10);
        chart.set_width(864).set_height(518);
        for (c, series) in self.series.iter().enumerate() {
            let col = c as u16 + 1;
            sheet.write_string(0, col, &series.name).map_err(invalid)?;
            for (r, value) in series.values.iter().enumerate() {
                sheet
                    .write_number(r as u32 + 1, col, *value)
                    .map_err(invalid)?;
            }
            chart
                .add_series()
                .set_name(("Sheet1", 0, col))
                .set_categories(("Sheet1", 1, 0, self.categories.len() as u32, 0))
                .set_values(("Sheet1", 1, col, self.categories.len() as u32, col));
        }
        sheet
            .insert_chart(0, self.series.len() as u16 + 3, &chart)
            .map_err(invalid)?;
        let bytes = workbook.save_to_buffer().map_err(invalid)?;
        let mut archive = zip::ZipArchive::new(Cursor::new(&bytes)).map_err(invalid)?;
        let mut xml = String::new();
        archive
            .by_name("xl/charts/chart1.xml")
            .map_err(invalid)?
            .read_to_string(&mut xml)?;
        if !xml.contains("xmlns:r=") || !xml.contains("</c:chartSpace>") {
            return Err(invalid("Unexpected chart XML format"));
        }
        let xml = xml.replace("</c:chartSpace>", "<c:externalData r:id=\"rId1\"><c:autoUpdate val=\"0\"/></c:externalData></c:chartSpace>");
        Ok((xml.into_bytes(), bytes))
    }
}

pub(super) fn replace_chart_packages(
    bytes: Vec<u8>,
    packages: Vec<(Vec<u8>, Vec<u8>)>,
) -> Result<Vec<u8>> {
    if packages.is_empty() {
        return Ok(bytes);
    }
    let mut replacements = BTreeMap::new();
    for (i, (chart, workbook)) in packages.into_iter().enumerate() {
        replacements.insert(format!("ppt/charts/chart{}.xml", i + 1), chart);
        replacements.insert(
            format!("ppt/embeddings/Microsoft_Excel_Sheet{}.xlsx", i + 1),
            workbook,
        );
    }
    rewrite(bytes, replacements)
}

pub(super) fn rewrite(
    bytes: Vec<u8>,
    mut replacements: BTreeMap<String, Vec<u8>>,
) -> Result<Vec<u8>> {
    let mut input = zip::ZipArchive::new(Cursor::new(bytes)).map_err(invalid)?;
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for i in 0..input.len() {
        let file = input.by_index(i).map_err(invalid)?;
        if let Some(data) = replacements.remove(file.name()) {
            output
                .start_file(
                    file.name(),
                    zip::write::SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Deflated),
                )
                .map_err(invalid)?;
            output.write_all(&data)?;
        } else {
            output.raw_copy_file(file).map_err(invalid)?;
        }
    }
    if !replacements.is_empty() {
        return Err(invalid("Missing expected Office package parts"));
    }
    Ok(output.finish().map_err(invalid)?.into_inner())
}
