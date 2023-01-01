use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct PaginationProps {
    pub current: usize,
    pub page_size: usize,
    pub count: usize,
    pub onclick: Callback<usize>,
}

#[function_component(Pagination)]
pub fn pagination(props: &PaginationProps) -> Html {
    let mut pages = props.count / props.page_size;
    if props.count % props.page_size != 0 {
        pages += 1;
    }

    let pages = (0..pages).into_iter().map(|page| {
        let onclick = props.onclick.clone();
        let onclick = Callback::from(move |_| {
            onclick.emit(page);
        });

        let class = if page == props.current {
            "page-item active"
        } else {
            "page-item"
        };

        html!(<li { class }>
            <a class="page-link" { onclick }>{ page + 1 }</a>
        </li>)
    });

    html!(<ul class="pagination">
        { for pages }
    </ul>)
}
