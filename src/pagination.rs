//! This modules defines the common functionality for paging data.

/// The config for pagination
#[derive(Debug, Clone)]
pub struct PaginationConfig {
    /// The page number to default to when not specified in a request.
    pub default_page: u64,
    /// The maximum transactions to display per page when not specified in a request.
    pub default_page_size: u64,
    /// The maximum number of pages to show in the pagination indicator.
    pub max_pages: u64,
}

impl Default for PaginationConfig {
    fn default() -> Self {
        Self {
            default_page: 1,
            default_page_size: 20,
            max_pages: 5,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum PaginationIndicator {
    Page(u64),
    CurrPage(u64),
    Ellipsis,
    NextButton(u64),
    BackButton(u64),
}

pub fn create_pagination_indicators(
    curr_page: u64,
    page_count: u64,
    max_pages: u64,
) -> Vec<PaginationIndicator> {
    let map_page = |page| {
        if page == curr_page {
            PaginationIndicator::CurrPage(page)
        } else {
            PaginationIndicator::Page(page)
        }
    };

    let mut indicators: Vec<PaginationIndicator> = if page_count <= max_pages {
        (1..=page_count).map(map_page).collect()
    } else if curr_page <= (max_pages / 2) {
        (1..=max_pages).map(map_page).collect()
    } else if curr_page > (page_count - max_pages / 2) {
        ((page_count - max_pages + 1)..=page_count)
            .map(map_page)
            .collect()
    } else {
        ((curr_page - max_pages / 2)..=(curr_page + max_pages / 2))
            .map(map_page)
            .collect()
    };

    if page_count > max_pages {
        if curr_page > (max_pages / 2) + 1 {
            indicators.insert(0, PaginationIndicator::Page(1));
            indicators.insert(1, PaginationIndicator::Ellipsis);
        }

        if curr_page < (page_count - max_pages / 2) {
            indicators.push(PaginationIndicator::Ellipsis);
            indicators.push(PaginationIndicator::Page(page_count));
        }
    }

    if curr_page > 1 {
        indicators.insert(0, PaginationIndicator::BackButton(curr_page - 1));
    }

    if curr_page < page_count {
        indicators.push(PaginationIndicator::NextButton(curr_page + 1));
    }

    indicators
}

#[cfg(test)]
mod tests {
    use crate::pagination::{PaginationIndicator, create_pagination_indicators};

    #[test]
    fn shows_all_pages() {
        let max_pages = 5;
        let page_count = 5;
        let curr_page = 1;
        let want = [
            PaginationIndicator::CurrPage(1),
            PaginationIndicator::Page(2),
            PaginationIndicator::Page(3),
            PaginationIndicator::Page(4),
            PaginationIndicator::Page(5),
            PaginationIndicator::NextButton(2),
        ];

        let got = create_pagination_indicators(curr_page, page_count, max_pages);

        assert_eq!(want, got.as_slice());
    }

    #[test]
    fn shows_page_subset_on_left() {
        let max_pages = 5;
        let page_count = 10;
        let curr_page = 1;
        let want = [
            PaginationIndicator::CurrPage(1),
            PaginationIndicator::Page(2),
            PaginationIndicator::Page(3),
            PaginationIndicator::Page(4),
            PaginationIndicator::Page(5),
            PaginationIndicator::Ellipsis,
            PaginationIndicator::Page(10),
            PaginationIndicator::NextButton(2),
        ];

        let got = create_pagination_indicators(curr_page, page_count, max_pages);

        assert_eq!(want, got.as_slice());
    }

    #[test]
    fn shows_both_buttons_and_trailing_ellipsis() {
        let max_pages = 5;
        let page_count = 10;
        let curr_page = 3;
        let want = [
            PaginationIndicator::BackButton(2),
            PaginationIndicator::Page(1),
            PaginationIndicator::Page(2),
            PaginationIndicator::CurrPage(3),
            PaginationIndicator::Page(4),
            PaginationIndicator::Page(5),
            PaginationIndicator::Ellipsis,
            PaginationIndicator::Page(10),
            PaginationIndicator::NextButton(4),
        ];

        let got = create_pagination_indicators(curr_page, page_count, max_pages);

        assert_eq!(want, got.as_slice());
    }

    #[test]
    fn shows_page_subset_on_right() {
        let max_pages = 5;
        let page_count = 10;
        let curr_page = 10;
        let want = [
            PaginationIndicator::BackButton(9),
            PaginationIndicator::Page(1),
            PaginationIndicator::Ellipsis,
            PaginationIndicator::Page(6),
            PaginationIndicator::Page(7),
            PaginationIndicator::Page(8),
            PaginationIndicator::Page(9),
            PaginationIndicator::CurrPage(10),
        ];

        let got = create_pagination_indicators(curr_page, page_count, max_pages);

        assert_eq!(want, got.as_slice());
    }

    #[test]
    fn shows_both_buttons_and_leading_ellipsis() {
        let max_pages = 5;
        let page_count = 10;
        let curr_page = 8;
        let want = [
            PaginationIndicator::BackButton(7),
            PaginationIndicator::Page(1),
            PaginationIndicator::Ellipsis,
            PaginationIndicator::Page(6),
            PaginationIndicator::Page(7),
            PaginationIndicator::CurrPage(8),
            PaginationIndicator::Page(9),
            PaginationIndicator::Page(10),
            PaginationIndicator::NextButton(9),
        ];

        let got = create_pagination_indicators(curr_page, page_count, max_pages);

        assert_eq!(want, got.as_slice());
    }

    #[test]
    fn pagination_indicator_shows_page_subset_in_center() {
        let max_pages = 5;
        let page_count = 10;
        let curr_page = 5;
        let want = [
            PaginationIndicator::BackButton(4),
            PaginationIndicator::Page(1),
            PaginationIndicator::Ellipsis,
            PaginationIndicator::Page(3),
            PaginationIndicator::Page(4),
            PaginationIndicator::CurrPage(5),
            PaginationIndicator::Page(6),
            PaginationIndicator::Page(7),
            PaginationIndicator::Ellipsis,
            PaginationIndicator::Page(10),
            PaginationIndicator::NextButton(6),
        ];

        let got = create_pagination_indicators(curr_page, page_count, max_pages);

        assert_eq!(want, got.as_slice());
    }
}
