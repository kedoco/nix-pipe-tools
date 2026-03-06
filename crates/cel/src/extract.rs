use crate::parse::Table;
use crate::selector;

/// Extracted table with resolved columns
pub struct Extracted {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

/// Apply column selectors to a parsed table
pub fn extract(
    table: &Table,
    selectors: &[selector::Selector],
    exclude: bool,
) -> Result<Extracted, String> {
    let num_cols = if table.headers.is_empty() {
        table.rows.first().map_or(0, |r| r.len())
    } else {
        table.headers.len()
    };

    if num_cols == 0 {
        return Ok(Extracted {
            headers: Vec::new(),
            rows: Vec::new(),
        });
    }

    let indices = selector::resolve(selectors, &table.headers, num_cols, exclude)?;

    let headers: Vec<String> = indices
        .iter()
        .map(|&i| {
            table
                .headers
                .get(i)
                .cloned()
                .unwrap_or_else(|| format!("col{}", i + 1))
        })
        .collect();

    let rows: Vec<Vec<String>> = table
        .rows
        .iter()
        .map(|row| {
            indices
                .iter()
                .map(|&i| row.get(i).cloned().unwrap_or_default())
                .collect()
        })
        .collect();

    Ok(Extracted { headers, rows })
}
