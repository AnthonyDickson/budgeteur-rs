use scraper::{ElementRef, Html, Selector};

#[track_caller]
pub(crate) fn must_get_form(html: &Html) -> ElementRef<'_> {
    html.select(&Selector::parse("form").unwrap())
        .next()
        .expect("No form found")
}

#[track_caller]
pub(crate) fn assert_hx_endpoint(form: &ElementRef<'_>, endpoint: &str, attribute: &str) {
    let hx_post = form
        .value()
        .attr(attribute)
        .unwrap_or_else(|| panic!("{attribute} attribute missing"));

    assert_eq!(
        hx_post, endpoint,
        "want form with attribute {attribute}=\"{endpoint}\", got {hx_post:?}"
    );
}

#[track_caller]
pub(crate) fn assert_form_input(form: &ElementRef<'_>, name: &str, type_: &str) {
    for input in form.select(&Selector::parse("input").unwrap()) {
        let input_name = input.value().attr("name").unwrap_or_default();

        if input_name == name {
            let input_type = input.value().attr("type").unwrap_or_default();
            let input_required = input.value().attr("required");

            assert_eq!(
                input_type, type_,
                "want input with type \"{type_}\", got {input_type:?}"
            );

            assert!(
                input_required.is_some(),
                "want input with name {name} to have the required attribute but got none"
            );

            return;
        }
    }

    panic!("No input found with name \"{name}\" and type \"{type_}\"");
}

#[track_caller]
pub(crate) fn assert_form_input_with_value(
    form: &ElementRef<'_>,
    name: &str,
    type_: &str,
    value: &str,
) {
    for input in form.select(&Selector::parse("input").unwrap()) {
        let input_name = input.value().attr("name").unwrap_or_default();

        if input_name == name {
            let input_type = input.value().attr("type").unwrap_or_default();
            let input_value = input.value().attr("value").unwrap_or_default();
            let input_required = input.value().attr("required");

            assert_eq!(
                input_type, type_,
                "want input with type \"{type_}\", got {input_type:?}"
            );
            assert_eq!(
                input_value, value,
                "want input with value \"{value}\", got {input_value:?}"
            );
            assert!(
                input_required.is_some(),
                "want input with name {name} to have the required attribute but got none"
            );

            return;
        }
    }

    panic!("No input found with name \"{name}\" and type \"{type_}\"");
}

#[track_caller]
pub(crate) fn assert_form_submit_button(form: &ElementRef<'_>) {
    let submit_button = form
        .select(&Selector::parse("button").unwrap())
        .next()
        .expect("No button found");

    assert_eq!(
        submit_button.value().attr("type").unwrap_or_default(),
        "submit",
        "want submit button with type=\"submit\""
    );
}

#[track_caller]
pub(crate) fn assert_form_submit_button_with_text(form: &ElementRef<'_>, text: &str) {
    let submit_button = form
        .select(&Selector::parse("button").unwrap())
        .next()
        .expect("No button found");

    assert_eq!(
        submit_button.value().attr("type").unwrap_or_default(),
        "submit",
        "want submit button with type=\"submit\""
    );
    let got_text = submit_button.text().collect::<Vec<_>>().join("");
    let got_text = got_text.trim();
    assert_eq!(text, got_text);
}

#[track_caller]
pub(crate) fn assert_form_error_message(form: &ElementRef<'_>, want_error_message: &str) {
    let p = Selector::parse("p").unwrap();
    let error_message = form
        .select(&p)
        .next()
        .expect("No error message found")
        .text()
        .collect::<Vec<_>>()
        .join("");
    let got_error_message = error_message.trim();

    assert_eq!(want_error_message, got_error_message);
}
