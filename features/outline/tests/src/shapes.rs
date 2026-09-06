pub fn shaped(drawn: &str) -> String {
    let lines: Vec<&str> = drawn
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    let flush = lines
        .iter()
        .map(|line| line.len() - line.trim_start().len())
        .min()
        .unwrap_or_default();

    lines
        .iter()
        .map(|line| &line[flush..])
        .collect::<Vec<_>>()
        .join("\n")
}
