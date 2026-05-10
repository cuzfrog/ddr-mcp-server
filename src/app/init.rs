//! Config file initialisation helpers.
//!
//! Responsible for generating a default `docent.toml` and merging new config
//! fields into an existing file while preserving user customisations.

/// Merge existing config values into the default template.
///
/// For each section in the existing config, the corresponding section in the
/// template is scanned and matching keys are replaced with the user's value.
pub(crate) fn merge_toml(template: &str, existing: &str) -> anyhow::Result<String> {
    let existing_root: toml::Value = toml::from_str(existing)
        .map_err(|e| anyhow::anyhow!("Failed to parse existing config: {}", e))?;

    let mut result = template.to_string();

    if let toml::Value::Table(existing_table) = &existing_root {
        for (section_name, section_value) in existing_table {
            if let toml::Value::Table(keys) = section_value {
                for (key, existing_val) in keys {
                    if let Some(new_result) =
                        replace_value_in_text(&result, section_name, key, existing_val)
                    {
                        result = new_result;
                    }
                }
            }
        }
    }

    Ok(result)
}

fn replace_value_in_text(
    text: &str,
    section_name: &str,
    key: &str,
    existing_val: &toml::Value,
) -> Option<String> {
    let header = format!("[{}]", section_name);
    let new_val_str = format_toml_inline(existing_val);
    let mut in_section = false;
    let mut result = String::new();
    let mut replaced = false;

    for line in text.lines() {
        let trimmed = line.trim();

        if !replaced && in_section {
            if trimmed.starts_with('[') && trimmed.ends_with(']') && !trimmed.starts_with("[[") {
                in_section = false;
            } else if let Some(eq_pos) = trimmed.find('=') {
                let line_key = trimmed[..eq_pos].trim();
                if line_key == key {
                    let line_eq_pos = line.find('=').unwrap();
                    let before_eq = &line[..line_eq_pos + 1];
                    let after_eq = &line[line_eq_pos + 1..];
                    let val_body_start = after_eq.find(|c: char| !c.is_whitespace()).unwrap_or(0);
                    let trailing_after_value = &after_eq[val_body_start..];
                    let comment_idx = find_comment_start(trailing_after_value);
                    let new_line = match comment_idx {
                        Some(ci) => {
                            let val_content_end = trailing_after_value[..ci].trim_end().len();
                            let spacing_and_comment = &trailing_after_value[val_content_end..];
                            format!(
                                "{}{}{}{}",
                                before_eq,
                                &after_eq[..val_body_start],
                                new_val_str,
                                spacing_and_comment
                            )
                        }
                        None => {
                            format!("{}{}{}", before_eq, &after_eq[..val_body_start], new_val_str)
                        }
                    };
                    result.push_str(&new_line);
                    result.push('\n');
                    replaced = true;
                    continue;
                }
            }
        }

        if !replaced && trimmed == header.as_str() {
            in_section = true;
        }

        result.push_str(line);
        result.push('\n');
    }

    if replaced { Some(result) } else { None }
}

fn find_comment_start(s: &str) -> Option<usize> {
    let mut in_quotes = false;
    let mut escaped = false;
    for (i, ch) in s.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '"' => in_quotes = !in_quotes,
            '#' if !in_quotes => return Some(i),
            _ => {}
        }
    }
    None
}

fn format_toml_inline(val: &toml::Value) -> String {
    let mut table = toml::value::Table::new();
    table.insert("_".to_string(), val.clone());
    let serialized = toml::to_string(&toml::Value::Table(table)).unwrap_or_default();
    serialized
        .trim()
        .strip_prefix("_ = ")
        .unwrap_or("")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_inserts_missing_key_in_correct_section() {
        let existing = r#"
[index]
embedding_model = "BGESmallENV15Q"
persist_path = "./.docent-index"
chunk_overlap = 64
max_size_mb = 512
"#;

        let template = crate::config::defaults::DEFAULT_TEMPLATE;
        let merged = merge_toml(template, existing).unwrap();
        let index_pos = merged.find("[index]").unwrap();
        let next_section_pos = merged[index_pos + 1..]
            .find("\n[")
            .map(|p| index_pos + 1 + p)
            .unwrap_or(merged.len());
        let index_section = &merged[index_pos..next_section_pos];

        assert!(
            index_section.contains("chunk_size"),
            "chunk_size should appear inside the [index] section, got:\n{}",
            merged
        );

        let after_last_section = &merged[next_section_pos..];
        assert!(
            !after_last_section.contains("chunk_size"),
            "chunk_size should not appear after other sections, got:\n{}",
            merged
        );
    }
}
