use crate::types::Entry;

/// Print a table of processes that hold a resource (file/port query).
///
/// ```text
/// PID     PROCESS     USER     FD    MODE
/// 1234    python      kevin    3     rw
/// 5678    sqlite3     kevin    5     r
/// ```
pub fn print_process_table(entries: &[Entry], no_header: bool) {
    let headers = ["PID", "PROCESS", "USER", "FD", "MODE"];
    let rows: Vec<[&str; 5]> = entries
        .iter()
        .map(|e| [e.pid.as_str(), e.command.as_str(), e.user.as_str(), e.fd.as_str(), e.access.as_str()])
        .collect();

    print_table(&headers, &rows, no_header);
}

fn print_table<const N: usize>(headers: &[&str; N], rows: &[[&str; N]], no_header: bool) {
    // Calculate column widths
    let mut widths = [0usize; N];
    if !no_header {
        for (i, h) in headers.iter().enumerate() {
            widths[i] = h.len();
        }
    }
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if cell.len() > widths[i] {
                widths[i] = cell.len();
            }
        }
    }

    if !no_header {
        print_row(headers, &widths);
    }

    for row in rows {
        print_row(row, &widths);
    }
}

fn print_row<const N: usize>(cells: &[&str; N], widths: &[usize; N]) {
    let last = N - 1;
    for (i, cell) in cells.iter().enumerate() {
        if i > 0 {
            print!("    ");
        }
        if i == last {
            // Last column: no trailing padding
            print!("{}", cell);
        } else {
            print!("{:width$}", cell, width = widths[i]);
        }
    }
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Entry;

    fn make_entry(pid: &str, cmd: &str, user: &str, fd: &str, ft: &str, access: &str, name: &str) -> Entry {
        Entry {
            pid: pid.to_string(),
            command: cmd.to_string(),
            user: user.to_string(),
            fd: fd.to_string(),
            file_type: ft.to_string(),
            access: access.to_string(),
            name: name.to_string(),
        }
    }

    #[test]
    fn process_table_alignment() {
        // Just verify it doesn't panic — visual output is hard to unit test
        let entries = vec![
            make_entry("1234", "python", "kevin", "3", "REG", "rw", "/tmp/db"),
            make_entry("56789", "sqlite3", "root", "5", "REG", "r", "/tmp/db"),
        ];
        print_process_table(&entries, false);
        print_process_table(&entries, true);
    }

}
