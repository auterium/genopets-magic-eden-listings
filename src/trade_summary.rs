use super::Trade;
use rust_decimal::Decimal;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct TradeSummaryProps {
    pub trades: Option<Vec<Trade>>,
}

#[function_component(TradeSummary)]
pub fn trade_summary(props: &TradeSummaryProps) -> Html {
    let trades = match &props.trades {
        Some(trades) => trades,
        None => return html!(<></>),
    };

    let count = trades.len();

    let (sum_amount, sum_value) = trades.iter().fold(
        (Decimal::ZERO, Decimal::ZERO),
        |(mut sum_amount, mut sum_value), trade| {
            sum_amount += trade.amount;
            sum_value += trade.amount * trade.price;

            (sum_amount, sum_value)
        },
    );
    let avg_price = sum_value / sum_amount;

    let trades = trades.iter().map(|trade| {
        let id = format!("{}{}", trade.ts, trade.price);

        html!(<tr { id }>
            <td>{ trade.ts.trim_end_matches(".000") }</td>
            <td>{ trade.price.round_dp(3).to_string() }</td>
            <td>{ trade.amount.to_string() }</td>
        </tr>)
    });

    html!(<div style="height: 250px; overflow: auto">
        <b>{ "Trades: " }</b>{ count }<br/>
        <b>{ "Avg. price: "}</b>{ avg_price.round_dp(3).to_string() }<br/>
        <table class="table table-striped table-bordered">
            <thead>
                <tr>
                    <th>{ "Date" }</th>
                    <th>{ "Price" }</th>
                    <th>{ "Amount" }</th>
                </tr>
            </thead>
            <tbody>
                { for trades }
            </tbody>
        </table>
    </div>)
}
