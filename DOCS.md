# Developer Docs

## Error Handling

Page endpoints should rely `Error`'s `IntoResponse` implementation to render the HTML response.
Fragment endpoints should manually insert an error message into the form, or if there's no form use the `AlertTemplate`.
All errors should be logged at the source callsite, typically with `inspect_err`.
