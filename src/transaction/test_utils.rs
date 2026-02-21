use scraper::ElementRef;

#[track_caller]
pub fn assert_transaction_type_inputs(form: &ElementRef, checked_type: Option<&str>) {
    let selector = scraper::Selector::parse("input[type=radio][name=type_]").unwrap();
    let inputs = form.select(&selector).collect::<Vec<_>>();
    assert_eq!(
        inputs.len(),
        2,
        "want 2 transaction type inputs, got {}",
        inputs.len()
    );

    let mut values = inputs
        .iter()
        .filter_map(|input| input.value().attr("value"))
        .collect::<Vec<_>>();
    values.sort_unstable();
    assert_eq!(
        values,
        vec!["expense", "income"],
        "want transaction type values to be expense/income, got {values:?}"
    );

    let checked_count = inputs
        .iter()
        .filter(|input| input.value().attr("checked").is_some())
        .count();
    assert_eq!(
        checked_count, 1,
        "want exactly one transaction type input checked, got {checked_count}"
    );

    for input in &inputs {
        let required = input.value().attr("required");
        let input_name = input.value().attr("name").unwrap_or("type_");
        assert!(
            required.is_some(),
            "want {input_name} input to be required, got {required:?}"
        );
    }

    if let Some(checked_type) = checked_type {
        let expected_checked = inputs.iter().any(|input| {
            input.value().attr("value") == Some(checked_type)
                && input.value().attr("checked").is_some()
        });
        assert!(
            expected_checked,
            "want {checked_type} to be checked, but it was not"
        );
    }
}
