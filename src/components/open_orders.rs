use crate::utils::Listing;
use rust_decimal::{Decimal, MathematicalOps};
use solana_sdk::pubkey::Pubkey;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct OpenOrdersProps {
    pub orders: Vec<Listing>,
    pub owner_key: Option<Pubkey>,
}

#[function_component(OpenOrders)]
pub fn open_orders(props: &OpenOrdersProps) -> Html {
    let orders = props.orders.iter().filter_map(|listing| {
        match props.owner_key {
            Some(owner_key) if owner_key != listing.owner => return None,
            _ => {}
        }

        let price = compute_ui_price(listing.price);

        Some(html!(<tr key={ listing.key }>
            <td>{ price.round_dp(3).to_string() }</td>
            <td>{ listing.base_quantity }</td>
        </tr>))
    });

    html!(<div style="height: 250px; overflow: auto">
        <table class="table table-striped table-bordered">
            <thead>
                <tr>
                    <th>{ "Price" }</th>
                    <th>{ "Quantity" }</th>
                </tr>
            </thead>
            <tbody>
                { for orders }
            </tbody>
        </table>
    </div>)
}

fn compute_ui_price(price: u64) -> Decimal {
    // This is a weitd number. I would've expected it to be 9496 (5% + 0.04%)
    let fee_mult = Decimal::from_i128_with_scale(9520, 4);

    let quote_currency_multiplier = Decimal::from_i128_with_scale(1_000_000, 0);
    let price = Decimal::from_i128_with_scale(price as i128, 0);

    let numerator = price * quote_currency_multiplier;
    let denominator =
        Decimal::from_i128_with_scale(1_000_000_000, 0) * Decimal::TWO.powu(32) * fee_mult;

    numerator / denominator
}
