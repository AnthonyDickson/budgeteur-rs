use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Flex, Layout, Rect},
    style::{Color, Style, Styled, Stylize},
    symbols::Marker,
    text::{Line, Text},
    widgets::{
        Axis, Block, Cell, Chart, Dataset, GraphType, LegendPosition, LineGauge, Paragraph, Row,
        Scrollbar, ScrollbarOrientation, ScrollbarState, Table, TableState, Wrap,
    },
};
use time::macros::format_description;

use crate::{
    key_binding::KeyBinding,
    request::{self, RequestContext},
    runtime::Cmd,
};
use budgeteur_shared::{
    currency::{format_currency, format_currency_rounded},
    dashboard::{
        DashboardData, ExpensesByTagStats, NetIncomeStats, NetWorthStats, SavingsStats,
        SpendingPaceStats, UntaggedTransaction,
    },
};

// ----------------------------------------------------------------------------
// Model
// ----------------------------------------------------------------------------

#[allow(clippy::large_enum_variant)]
enum Status {
    NoData,
    Loaded(DashboardData),
    Error(String),
}
pub struct Model {
    status: Status,
    request_ctx: RequestContext,
}

#[derive(Clone, Copy, PartialEq)]
enum DashboardWidget {
    NetWorth,
    NetIncome,
    ExpensesByTag,
    SpendingPace,
    Savings,
    UntaggedTransactions,
}

impl DashboardWidget {
    fn first() -> Self {
        Self::NetWorth
    }

    fn last() -> Self {
        Self::UntaggedTransactions
    }

    /// Defines the UI element to focus when the user presses 'TAB'. When called
    /// on the last element, the first element is returned (wraps around).
    fn next(&self) -> Self {
        match self {
            Self::NetWorth => Self::NetIncome,
            Self::NetIncome => Self::ExpensesByTag,
            Self::ExpensesByTag => Self::SpendingPace,
            Self::SpendingPace => Self::Savings,
            Self::Savings => Self::UntaggedTransactions,
            Self::UntaggedTransactions => Self::NetWorth,
        }
    }

    /// Defines the UI element to focus when the user presses 'SHIFT + TAB'
    /// Defines the UI element to focus when the user presses 'SHIFT+TAB'. When
    /// called on the first element, the last element is returned (wraps around).
    fn prev(&self) -> Self {
        match self {
            Self::NetWorth => Self::UntaggedTransactions,
            Self::NetIncome => Self::NetWorth,
            Self::ExpensesByTag => Self::NetIncome,
            Self::SpendingPace => Self::ExpensesByTag,
            Self::Savings => Self::SpendingPace,
            Self::UntaggedTransactions => Self::Savings,
        }
    }
}

/// Ratatui widget state — held separately from [`Model`] to keep the model
/// pure (owned by the runtime, passed by `&mut` reference to `view`).
pub struct ViewState {
    untagged_table_state: TableState,
    focus: Option<DashboardWidget>,
    expense_scrollbar: ScrollbarState,
}

impl ViewState {
    pub fn new() -> Self {
        Self {
            untagged_table_state: TableState::default(),
            focus: None,
            expense_scrollbar: ScrollbarState::default(),
        }
    }
}

// ----------------------------------------------------------------------------
// Message
// ----------------------------------------------------------------------------

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum Message {
    FetchResult(Result<DashboardData, String>),
    Refresh,
    Confirm,
}

// ----------------------------------------------------------------------------
// Init
// ----------------------------------------------------------------------------

pub fn init(request_ctx: &RequestContext) -> (Model, Cmd<Message>, ViewState) {
    (
        Model {
            status: Status::NoData,
            request_ctx: request_ctx.clone(),
        },
        fetch_dashboard(
            request_ctx.base_url.clone(),
            request_ctx.signing_key_der.clone(),
        ),
        ViewState::new(),
    )
}

// ----------------------------------------------------------------------------
// Update
// ----------------------------------------------------------------------------

pub fn update(model: Model, msg: Message) -> (Model, Cmd<Message>) {
    match msg {
        Message::FetchResult(Ok(dashboard_data)) => (
            Model {
                status: Status::Loaded(dashboard_data),
                ..model
            },
            Cmd::none(),
        ),
        Message::FetchResult(Err(error)) => (
            Model {
                status: Status::Error(error),
                ..model
            },
            Cmd::none(),
        ),
        Message::Refresh => {
            let cmd = fetch_dashboard(
                model.request_ctx.base_url.clone(),
                model.request_ctx.signing_key_der.clone(),
            );
            (model, cmd)
        }
        Message::Confirm => {
            // TODO: Open tagging UI for selected transaction
            (model, Cmd::none())
        }
    }
}

/// Map a raw key event to an optional page message. Mutates [`ViewState`]
/// directly for navigation (j/k); only returns a message for actions that
/// need the async command pipeline.
pub fn handle_key(key: KeyCode, view_state: &mut ViewState) -> Option<Message> {
    // Block level bindings
    match view_state.focus {
        Some(DashboardWidget::UntaggedTransactions) => match key {
            KeyCode::Char('j') | KeyCode::Down => {
                view_state.untagged_table_state.select_next();
                return None;
            }
            KeyCode::Char('k') | KeyCode::Up => {
                view_state.untagged_table_state.select_previous();
                return None;
            }
            KeyCode::Char('g') => {
                view_state.untagged_table_state.select_first();
                return None;
            }
            KeyCode::Char('G') => {
                view_state.untagged_table_state.select_last();
                return None;
            }
            KeyCode::Enter => return Some(Message::Confirm),
            _ => {}
        },
        Some(DashboardWidget::ExpensesByTag) => match key {
            KeyCode::Char('j') | KeyCode::Down => {
                view_state.expense_scrollbar.next();
                return None;
            }
            KeyCode::Char('k') | KeyCode::Up => {
                view_state.expense_scrollbar.prev();
                return None;
            }
            KeyCode::Char('g') => {
                view_state.expense_scrollbar.first();
                return None;
            }
            KeyCode::Char('G') => {
                view_state.expense_scrollbar.last();
                return None;
            }
            _ => {}
        },
        Some(DashboardWidget::NetWorth)
        | Some(DashboardWidget::NetIncome)
        | Some(DashboardWidget::SpendingPace)
        | Some(DashboardWidget::Savings)
        | None => {}
    };

    // Page-global bindings
    match key {
        KeyCode::Tab => {
            view_state.focus = match view_state.focus {
                Some(focus) => Some(focus.next()),
                None => Some(DashboardWidget::first()),
            };
            None
        }
        KeyCode::BackTab => {
            view_state.focus = match view_state.focus {
                Some(focus) => Some(focus.prev()),
                None => Some(DashboardWidget::last()),
            };
            None
        }
        KeyCode::Char('r') => Some(Message::Refresh),
        _ => None,
    }
}

pub fn key_bindings(view_state: &ViewState) -> Vec<KeyBinding> {
    let page_global_bindings = vec![
        KeyBinding {
            key: "tab".to_owned(),
            description: "focus next".to_owned(),
        },
        KeyBinding {
            key: "shift+tab".to_owned(),
            description: "focus prev".to_owned(),
        },
        KeyBinding {
            key: "r".to_owned(),
            description: "refresh".to_owned(),
        },
    ];

    let focused_widget_bindings = match view_state.focus {
        Some(DashboardWidget::UntaggedTransactions) => {
            vec![
                KeyBinding {
                    key: "j/↓".to_owned(),
                    description: "go down one row".to_owned(),
                },
                KeyBinding {
                    key: "k/↑".to_owned(),
                    description: "go up one row".to_owned(),
                },
                KeyBinding {
                    key: "g".to_owned(),
                    description: "go to first row".to_owned(),
                },
                KeyBinding {
                    key: "G".to_owned(),
                    description: "go to last row".to_owned(),
                },
            ]
        }
        Some(DashboardWidget::ExpensesByTag) => {
            vec![
                KeyBinding {
                    key: "j/↓".to_owned(),
                    description: "go down one row".to_owned(),
                },
                KeyBinding {
                    key: "k/↑".to_owned(),
                    description: "go up one row".to_owned(),
                },
                KeyBinding {
                    key: "g".to_owned(),
                    description: "go to first row".to_owned(),
                },
                KeyBinding {
                    key: "G".to_owned(),
                    description: "go to last row".to_owned(),
                },
            ]
        }
        _ => Vec::new(),
    };

    focused_widget_bindings
        .into_iter()
        .chain(page_global_bindings)
        .collect()
}

// ----------------------------------------------------------------------------
// View
// ----------------------------------------------------------------------------

pub fn view(model: &Model, view_state: &mut ViewState, area: Rect, f: &mut Frame) {
    match &model.status {
        Status::NoData => {
            let block = Block::bordered().title("Dashboard");
            let text_area = block
                .inner(area)
                .centered(Constraint::Length(7), Constraint::Length(1));
            f.render_widget(block, area);

            let paragraph = Paragraph::new(Text::from("NO DATA").centered());
            f.render_widget(paragraph, text_area);
        }
        Status::Loaded(data) => {
            let [first_row, second_row] = Layout::vertical([Constraint::Fill(1); 2]).areas(area);

            let [net_worth_cell, net_income_cell, tags_cell] =
                Layout::horizontal([Constraint::Fill(1); 3]).areas(first_row);

            draw_net_worth_block(&data.net_worth, view_state, net_worth_cell, f);
            draw_net_income_block(&data.net_income, view_state, net_income_cell, f);
            draw_expenses_by_tag_block(&data.expenses_by_tag, view_state, tags_cell, f);

            let [spending_cell, savings_cell, untagged_cell] =
                Layout::horizontal([Constraint::Fill(1); 3]).areas(second_row);

            draw_spending_block(&data.spending_pace, view_state, spending_cell, f);
            draw_savings_block(&data.savings, view_state, savings_cell, f);
            draw_untagged_block(&data.untagged_transactions, view_state, untagged_cell, f);
        }
        Status::Error(error) => {
            let paragraph = Paragraph::new(Text::from(vec![
                Line::from("Error".bold()),
                Line::raw(error),
            ]))
            .wrap(Wrap { trim: true });

            let block = Block::bordered().title("Dashboard");
            let text_area = block
                .inner(area)
                .centered(Constraint::Percentage(40), Constraint::Percentage(30));
            f.render_widget(block, area);

            f.render_widget(paragraph, text_area)
        }
    }
}

fn draw_net_worth_block(data: &NetWorthStats, view_state: &ViewState, area: Rect, f: &mut Frame) {
    let block = Block::bordered()
        .title("Net Worth")
        .border_style(block_border_style(
            view_state.focus,
            DashboardWidget::NetWorth,
        ));
    f.render_widget(block.clone(), area);

    let layout = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(5),
    ]);
    let [amount_area, trend_area, _, chart_area] = block.inner(area).layout(&layout);

    let amount = Paragraph::new(format_currency_rounded(data.amount).gray()).right_aligned();
    f.render_widget(amount, amount_area);

    let trend_style = amount_style(data.trend);
    let trend_prefix = match data.trend {
        x if x > 0.0 => "↑ ",
        x if x < 0.0 => "↓ ",
        _ => "",
    };
    let trend = Paragraph::new(Line::from(vec![
        "TTM".dark_gray(),
        " ".into(),
        format!(
            "{trend_prefix}{}",
            format_currency_rounded(data.trend.abs())
        )
        .set_style(trend_style),
    ]))
    .right_aligned();
    f.render_widget(trend, trend_area);

    draw_line_chart(&data.monthly, Color::Gray, chart_area, f);
}

fn draw_net_income_block(data: &NetIncomeStats, view_state: &ViewState, area: Rect, f: &mut Frame) {
    let block = Block::bordered()
        .title("Net Income")
        .border_style(block_border_style(
            view_state.focus,
            DashboardWidget::NetIncome,
        ));
    f.render_widget(block.clone(), area);

    let layout = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(5),
    ]);
    let [trend_area, avg_area, _, chart_area] = block.inner(area).layout(&layout);

    let trend = Text::from(Line::from(vec![
        "Last 28 Days ".dark_gray(),
        format_currency_rounded(data.last_28_days).set_style(amount_style(data.last_28_days)),
    ]))
    .right_aligned();
    f.render_widget(trend, trend_area);

    let avg = Paragraph::new(Line::from(vec![
        "Monthly avg ".dark_gray(),
        format_currency_rounded(data.monthly_avg).set_style(amount_style(data.monthly_avg)),
    ]))
    .right_aligned();
    f.render_widget(avg, avg_area);

    draw_line_chart(
        &data.monthly,
        amount_style(data.last_28_days),
        chart_area,
        f,
    );
}

fn draw_expenses_by_tag_block(
    data: &[ExpensesByTagStats],
    view_state: &mut ViewState,
    area: Rect,
    f: &mut Frame,
) {
    let block = Block::bordered()
        .title(vec![
            "Expenses by Tag".into(),
            " ".into(),
            "Last 28 Days".dark_gray(),
        ])
        .border_style(block_border_style(
            view_state.focus,
            DashboardWidget::ExpensesByTag,
        ));
    let block_inner_area = block.inner(area);
    f.render_widget(block, area);

    // Calculate Layout
    const TEXT_ROWS: u16 = 1;
    const GAUGE_ROWS: u16 = 1;
    const ITEM_HEIGHT: u16 = TEXT_ROWS + GAUGE_ROWS;
    const VERTICAL_SPACING: u16 = 1;
    let max_visible_items =
        ((block_inner_area.height) + VERTICAL_SPACING) / (ITEM_HEIGHT + VERTICAL_SPACING);
    let max_visible_items = max_visible_items.max(1); // Always show at least one item
    let max_visible_items = max_visible_items as usize;

    let mut draw_list = |visible_items: &[ExpensesByTagStats], content_area: Rect| {
        // List view, text row + gauge
        let rows = Layout::vertical(
            visible_items
                .iter()
                .map(|_| Constraint::Length(ITEM_HEIGHT)),
        )
        .spacing(VERTICAL_SPACING)
        .split(content_area);

        for (i, row) in visible_items.iter().enumerate() {
            let [text_row, gauge_row] = Layout::vertical([
                Constraint::Length(TEXT_ROWS),
                Constraint::Length(GAUGE_ROWS),
            ])
            .areas(rows[i]);

            let tag_name = Line::raw(&row.tag_name);
            let amount = Line::raw(format_currency_rounded(row.amount));
            let [tag_name_area, amount_area] = Layout::horizontal([
                Constraint::Min(tag_name.width() as u16),
                Constraint::Min(amount.width() as u16),
            ])
            .direction(Direction::Horizontal)
            .flex(Flex::SpaceBetween)
            .areas(text_row);

            f.render_widget(Paragraph::new(tag_name).left_aligned(), tag_name_area);
            f.render_widget(Paragraph::new(amount).right_aligned(), amount_area);

            let gauge = LineGauge::default()
                .unfilled_style(Style::new().dark_gray())
                .filled_style(Style::new().blue())
                .ratio(row.ratio_of_expense)
                .label(format!("{:>2.0}%", row.ratio_of_expense * 100.0).dark_gray());
            f.render_widget(gauge, gauge_row);
        }
    };

    if max_visible_items >= data.len() {
        draw_list(data, block_inner_area);
    } else {
        let [content_area, scrollbar_area] = block_inner_area.layout(&Layout::horizontal([
            Constraint::Fill(1),
            Constraint::Length(2),
        ]));

        // Update scrollbar state
        view_state.expense_scrollbar = view_state
            .expense_scrollbar
            .content_length(data.len())
            .viewport_content_length(max_visible_items);

        // Select visible items
        let first_item = view_state.expense_scrollbar.get_position();
        let last_item = (first_item + max_visible_items).min(data.len());
        let visible_items = &data[first_item..last_item];

        draw_list(visible_items, content_area);

        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
        f.render_stateful_widget(scrollbar, scrollbar_area, &mut view_state.expense_scrollbar);
    };
}

fn draw_spending_block(
    data: &SpendingPaceStats,
    view_state: &ViewState,
    area: Rect,
    f: &mut Frame,
) {
    let block = Block::bordered()
        .title("Spending Pace")
        .border_style(block_border_style(
            view_state.focus,
            DashboardWidget::SpendingPace,
        ));
    f.render_widget(block.clone(), area);

    let layout = Layout::vertical([Constraint::Length(3), Constraint::Min(5)]);
    let [amount_area, chart_area] = block.inner(area).layout(&layout);

    let amount = match data.deviation_from_baseline_ratio {
        None => Text::from("NM".dark_gray()),
        Some(x) if x > 0.05 => Text::from(vec![
            Line::from(vec![
                "This month you are on track to spend ".dark_gray(),
                format_currency_rounded(data.deviation_from_baseline.abs()).red(),
            ]),
            Line::from(vec![
                "above".bold(),
                " your typical ".into(),
                format_currency_rounded(data.mean_monthly_expenses).into(),
            ])
            .dark_gray(),
        ]),
        Some(x) if x < -0.05 => Text::from(vec![
            Line::from(vec![
                "This month you are on track to spend ".dark_gray(),
                format_currency_rounded(data.deviation_from_baseline.abs()).gray(),
            ]),
            Line::from(vec![
                "below".bold(),
                " your typical ".into(),
                format_currency_rounded(data.mean_monthly_expenses).into(),
                " 🎉".into(),
            ])
            .dark_gray(),
        ]),
        _ => Text::from(Line::from(vec![
            "This month you are on track to spend your typical ".into(),
            format_currency_rounded(data.mean_monthly_expenses).into(),
        ]))
        .dark_gray(),
    };
    let amount = amount.right_aligned();
    f.render_widget(amount, amount_area);

    let historical_data = data
        .historical
        .iter()
        .copied()
        .enumerate()
        .map(|(i, value)| (i as f64, value))
        .collect::<Vec<_>>();

    let current_month_data = data
        .current
        .iter()
        .copied()
        .enumerate()
        .map(|(i, value)| (i as f64, value))
        .collect::<Vec<_>>();

    let min_amount = data
        .historical
        .iter()
        .chain(data.current.iter())
        .copied()
        .min_by(f64::total_cmp)
        .unwrap_or(0.0);

    let max_amount = data
        .historical
        .iter()
        .chain(data.current.iter())
        .copied()
        .max_by(f64::total_cmp)
        .unwrap_or(0.0);

    let datasets = [
        // Baseline
        Dataset::default()
            .name("Historical Spending")
            .marker(Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Color::DarkGray)
            .data(&historical_data),
        // Actual spending dataset must come after baseline so that it is drawn over the baseline line chart
        Dataset::default()
            .name("Spending MTD")
            .marker(Marker::Braille)
            .graph_type(GraphType::Line)
            .style(
                if let Some(deviation_ratio) = data.deviation_from_baseline_ratio
                    && deviation_ratio > 0.05
                {
                    Color::Red
                } else {
                    Color::Gray
                },
            )
            .data(&current_month_data),
    ];

    let x_axis =
        Axis::default().bounds([0.0, historical_data.last().map(|(i, _)| *i).unwrap_or(0.0)]);
    let y_axis = Axis::default().bounds([min_amount, max_amount]);
    let chart = Chart::new(datasets.into())
        .x_axis(x_axis)
        .y_axis(y_axis)
        .legend_position(Some(LegendPosition::TopLeft));
    f.render_widget(chart, chart_area);
}

fn draw_savings_block(data: &SavingsStats, view_state: &ViewState, area: Rect, f: &mut Frame) {
    let block = Block::bordered()
        .title("Savings")
        .border_style(block_border_style(
            view_state.focus,
            DashboardWidget::Savings,
        ));
    f.render_widget(block.clone(), area);

    let layout = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(5),
    ]);
    let [amount_area, months_savings_area, trend_area, chart_area] =
        block.inner(area).layout(&layout);

    // Current Savings
    let savings_amount = Text::from(format_currency_rounded(data.amount)).right_aligned();
    f.render_widget(savings_amount, amount_area);

    // Months of Savings
    let trend_style = amount_style(data.trend);
    let trend_prefix = match data.trend {
        x if x > 0.0 => "↑ ",
        x if x < 0.0 => "↓ ",
        _ => "",
    };

    let months_savings = Line::from(vec![
        "Equivalent to ".dark_gray(),
        data.months_of_savings.to_string().gray(),
        " Months".gray(),
    ])
    .right_aligned();
    f.render_widget(months_savings, months_savings_area);

    // Trend
    let savings_trend = Line::from(vec![
        "Last 3 Months ".dark_gray(),
        trend_prefix.set_style(trend_style),
        format_currency_rounded(data.trend.abs()).set_style(trend_style),
    ])
    .right_aligned();
    f.render_widget(savings_trend, trend_area);

    // Chart
    draw_line_chart(&data.monthly, Color::Gray, chart_area, f);
}

fn draw_untagged_block(
    data: &[UntaggedTransaction],
    view_state: &mut ViewState,
    area: Rect,
    f: &mut Frame,
) {
    let block = Block::bordered()
        .title(format!("Untagged Transactions ({})", data.len()))
        .border_style(block_border_style(
            view_state.focus,
            DashboardWidget::UntaggedTransactions,
        ));
    let table_area = block.inner(area);
    f.render_widget(block, area);

    let date_format = format_description!("[day padding:zero] [month repr:short] [year]");
    let row_data: Vec<(String, String, &str)> = data
        .iter()
        .map(|t| {
            let date = t.date.format(date_format).expect("Could not format dates");
            let amount = format_currency(t.amount);
            (date, amount, t.description.as_ref())
        })
        .collect();
    let max_amount_width = row_data
        .iter()
        .map(|(_, amount, _)| amount.len())
        .max()
        // 5 for width of "Amount"
        .unwrap_or(5) as u16;

    let rows = row_data.into_iter().map(|(date, amount, description)| {
        Row::new([
            Cell::from(date),
            Cell::from(Line::from(amount).right_aligned()),
            Cell::from(description),
        ])
    });

    let header = Row::new([
        Cell::from("Date"),
        Cell::from(Line::from("Amount").right_aligned()),
        Cell::from("Description"),
    ])
    .style(Style::new().bold());
    let widths = [
        Constraint::Length(11),
        Constraint::Length(max_amount_width),
        Constraint::Fill(1),
    ];
    let table = Table::new(rows, widths)
        .header(header)
        .column_spacing(2)
        .gray()
        .row_highlight_style(Style::new().reversed().bold());

    f.render_stateful_widget(table, table_area, &mut view_state.untagged_table_state);
}

fn draw_line_chart(values: &[f64], style: Color, area: Rect, f: &mut Frame) {
    let chart_data: Vec<(f64, f64)> = values
        .iter()
        .copied()
        .enumerate()
        .map(|(i, v)| (i as f64, v))
        .collect();

    let min = values.iter().copied().min_by(f64::total_cmp).unwrap_or(0.0);
    let max = values.iter().copied().max_by(f64::total_cmp).unwrap_or(0.0);

    let dataset = Dataset::default()
        .marker(Marker::Braille)
        .graph_type(GraphType::Line)
        .style(style)
        .data(&chart_data);

    let x_axis = Axis::default().bounds([0.0, chart_data.last().map(|(i, _)| *i).unwrap_or(0.0)]);
    let y_axis = Axis::default().bounds([min, max]);
    let chart = Chart::new(vec![dataset]).x_axis(x_axis).y_axis(y_axis);
    f.render_widget(chart, area);
}

/// Choose a foreground color based on whether `value` is negative or not.
fn amount_style(value: f64) -> Color {
    if value < 0.0 { Color::Red } else { Color::Gray }
}

fn block_border_style(focused: Option<DashboardWidget>, current: DashboardWidget) -> Style {
    if Some(current) == focused {
        Style::new().light_blue().bold()
    } else {
        Style::new().gray().not_bold()
    }
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

fn fetch_dashboard(server_url: String, signing_key_der: Vec<u8>) -> Cmd<Message> {
    Cmd::from(async move {
        let header = match request::sign_auth_header(&signing_key_der) {
            Ok(h) => h,
            Err(e) => return Message::FetchResult(Err(e)),
        };

        let client = reqwest::Client::new();
        let url = format!("{server_url}{}", budgeteur_shared::routes::DASHBOARD);
        match client
            .get(&url)
            .header("Authorization", &header)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => match resp.json::<DashboardData>().await {
                Ok(data) => Message::FetchResult(Ok(data)),
                Err(e) => Message::FetchResult(Err(format!("could not parse dashboard: {e:?}"))),
            },
            Ok(resp) if resp.status().as_u16() == 401 => Message::FetchResult(Err(
                "authentication failed — is your public key registered on the server?".into(),
            )),
            Ok(resp) => Message::FetchResult(Err(format!("server returned {}", resp.status()))),
            Err(e) => Message::FetchResult(Err(format!("connection error: {e:#?}"))),
        }
    })
}
