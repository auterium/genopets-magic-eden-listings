mod components;
mod utils;

use self::components::open_orders::OpenOrders;
use self::components::pagination::{Pagination, PaginationProps};
use self::components::trade_summary::TradeSummary;
use self::utils::{Listing, Listings};
use dex_v4::state::{UserAccountHeader, UserAccount};
use gloo_net::http::Request;
use rust_decimal::Decimal;
use serde::{de, Deserialize};
use serde_json::json;
use solana_sdk::{account::Account, pubkey, pubkey::Pubkey};
use std::{collections::HashMap, str::FromStr};
use utils::Recipes;
use web_sys::HtmlInputElement;
use yew::prelude::*;

#[allow(unused)]
macro_rules! console_log {
    ($($t:tt)*) => (web_sys::console::log_1(&format_args!($($t)*).to_string().into()))
}

// Workaround to use the macro in other modules. Kudos to
// https://stackoverflow.com/questions/26731243/how-do-i-use-a-macro-across-module-files
#[allow(unused)]
pub(crate) use console_log;

const SERUM_V4: Pubkey = pubkey!("srmv4uTCPF81hWDaPyEN2mLZ8XbvzuEM6LsAxR8NpjU");
const PAGE_SIZE: usize = 25;

fn main() {
    yew::Renderer::<App>::new().render();
}

#[derive(Default, Clone)]
pub struct SearchForm {
    owner: NodeRef,
    title: NodeRef,
    collection: NodeRef,
}

#[derive(Default)]
pub struct SearchFormData {
    owner: Option<Pubkey>,
    title: String,
    collection: String,
}

pub struct App {
    orders: HashMap<String, Vec<Listing>>,
    trades: HashMap<String, Vec<Trade>>,
    token_prices: Option<(Decimal, Decimal)>,
    recipes: Recipes,
    markets: Vec<MagicEdenItem>,
    search_data: SearchFormData,
    search_form: SearchForm,
    page: usize,
    user_accounts: HashMap<Pubkey, UserAccountHeader>,
}

pub enum AppMsg {
    Orders(HashMap<String, Vec<Listing>>),
    Trades(HashMap<String, Vec<Trade>>),
    TokenPrices(Decimal, Decimal),
    Search(SearchFormData),
    Page(usize),
    UserAccounts(HashMap<Pubkey, UserAccountHeader>),
}

impl From<&SearchForm> for AppMsg {
    fn from(search_form: &SearchForm) -> Self {
        fn get_val(node_ref: &NodeRef) -> String {
            node_ref.cast::<HtmlInputElement>().unwrap().value()
        }

        let owner = get_val(&search_form.owner);
        let owner = (!owner.is_empty()).then(|| match Pubkey::from_str(&owner) {
            Ok(owner) => owner,
            Err(_) => Pubkey::default(),
        });

        let data = SearchFormData {
            title: get_val(&search_form.title),
            owner,
            collection: get_val(&search_form.collection),
        };

        AppMsg::Search(data)
    }
}

impl Component for App {
    type Message = AppMsg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let cb_orders = ctx.link().callback(|orders| AppMsg::Orders(orders));
        let cb_trades = ctx.link().callback(|trades| AppMsg::Trades(trades));
        let cb_token_prices = ctx
            .link()
            .callback(|(ki_price, gene_price)| AppMsg::TokenPrices(ki_price, gene_price));
        let genopets_sfts = include_str!("../collections/genopets_sfts.json");

        let markets: Vec<MagicEdenItem> = serde_json::from_str(genopets_sfts).unwrap();
        let recipes = Recipes::new(markets.clone());

        wasm_bindgen_futures::spawn_local(sync_markets(markets.clone(), cb_orders));
        wasm_bindgen_futures::spawn_local(fetch_trades(cb_trades));
        wasm_bindgen_futures::spawn_local(token_prices(cb_token_prices));

        Self {
            orders: HashMap::new(),
            trades: HashMap::new(),
            token_prices: None,
            recipes,
            markets,
            search_data: SearchFormData::default(),
            search_form: SearchForm::default(),
            page: 0,
            user_accounts: HashMap::new(),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            AppMsg::Orders(orders) => self.orders = orders,
            AppMsg::Trades(trades) => self.trades = trades,
            AppMsg::Search(data) => {
                let old_data = std::mem::replace(&mut self.search_data, data);

                let new_owner = match self.search_data.owner.as_ref() {
                    Some(new_owner) if new_owner.as_ref() != &[0u8; 32] => match old_data.owner.as_ref() {
                        None => Some(new_owner),
                        Some(old_owner) if new_owner != old_owner => Some(new_owner),
                        _ => None,
                    },
                    _ => None,
                };

                if let Some(new_owner) = new_owner {
                    let new_owner = new_owner.to_bytes();
                    let cb_accounts = ctx.link().callback(AppMsg::UserAccounts);

                    let accounts = self.markets
                        .iter()
                        .map(|item| {
                            let seeds: [&[u8]; 2] = [&item.market_address.to_bytes(), &new_owner];

                            Pubkey::find_program_address(&seeds, &SERUM_V4).0.to_string()
                        })
                        .collect();

                    wasm_bindgen_futures::spawn_local(sync_accounts(accounts, cb_accounts));
                }
            },
            AppMsg::Page(page) => self.page = page,
            AppMsg::TokenPrices(ki_price, gene_price) => {
                self.token_prices = Some((ki_price, gene_price));
            }
            AppMsg::UserAccounts(accounts) => {
                self.user_accounts = accounts;
            }
        }

        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let markets = self.markets.iter().filter_map(|item| {
            let orders = self.orders.get(&item.token_address)?;

            let title = item.token_title.to_lowercase();
            let search_title = self.search_data.title.to_lowercase();

            if !title.contains(&search_title) {
                return None;
            }

            if self.search_data.collection != "" && self.search_data.collection != item.collection {
                return None;
            }

            let owner_key = self.search_data.owner.map(|owner_key| {
                let seeds: &[&[u8]] = &[&item.market_address.to_bytes(), &owner_key.to_bytes()];

                Pubkey::find_program_address(seeds, &SERUM_V4).0
            });

            if let Some(owner_key) = &owner_key {
                if !self.user_accounts.contains_key(owner_key) {
                    orders.iter().find(|listing| &listing.owner == owner_key)?;
                }
            }

            Some((item, orders, owner_key))
        });

        let pagination_props = PaginationProps {
            current: self.page,
            count: markets.clone().count(),
            page_size: 25,
            onclick: ctx.link().callback(|page| AppMsg::Page(page)),
        };

        let markets = markets
            .skip(self.page * PAGE_SIZE)
            .take(PAGE_SIZE)
            .map(|(item, orders, owner_key)| {
                let trades = self.trades.get(&item.base_vault_address).cloned();

                let recipe = match self.recipes.get(&item.token_address) {
                    Some(recipe) => {
                        let reagents = recipe.reagents
                            .iter()
                            .map(|(reagent, amount)| html!(<li>{ format!("{} {}", amount, reagent) }</li>));

                        let total_cost = self.token_prices
                            .map(|(ki_price, gene_price)| {
                                let ki_cost = recipe.ki_cost * ki_price;
                                let gene_cost = recipe.gene_cost * gene_price;
                                let total_cost = ki_cost + gene_cost;

                                format!(" {} SOL", total_cost.round_dp(4))
                            })
                            .unwrap_or_default();

                        html!(<>
                            { total_cost }
                            <hr/>
                            <h6>{ "Recipe" }</h6>
                            <ul>
                                { for reagents }
                            </ul>
                        </>)
                    }
                    None => html!(<></>),
                };

                html!(<tr key={ item.token_address.clone() }>
                    <td>
                        <img src={ Some(item.token_image.clone()) } style="width: 230px; height: 230px" /><br/>
                        <a href={ format!("https://magiceden.io/sft/{}", item.market_address) } target="_blank">{ &item.token_title }</a>
                    </td>
                    <td>{ recipe }</td>
                    <td><OpenOrders orders={ orders.clone() } {owner_key} /></td>
                    <td><TradeSummary { trades } /></td>
                </tr>)
            });

        let pending_collect = (self.search_data.owner.is_some() && !self.user_accounts.is_empty()).then(|| {
            let rows = self.user_accounts
                .iter()
                .filter_map(|(key, account)| {
                    if account.quote_token_free == 0 {
                        return None;
                    }

                    let market = account.market.to_string();
                    let pending = Decimal::from_i128_with_scale(account.quote_token_free as i128, 9).round_dp(3);

                    Some(html!(<tr key={ key.to_string() }>
                        <td>
                            <a href={ format!("https://magiceden.io/sft/{market}") } target="_blank">{ market }</a>
                        </td>
                        <td>{ pending }</td>
                    </tr>))
                });

            html!(<table class="table table-striped table-bordered">
                <thead>
                    <tr>
                        <th>{ "Market" }</th>
                        <th>{ "Pending SOL collect" }</th>
                    </tr>
                </thead>
                <tbody>
                    { for rows }
                </tbody>
            </table>)
        });

        let search_form = self.search_form.clone();
        let oninput = ctx.link().callback(move |_| AppMsg::from(&search_form));

        let search_form = self.search_form.clone();
        let onchange = ctx.link().callback(move |_| AppMsg::from(&search_form));

        html!(<div class="container">
            <div class="row">
                <div class="form-group col-md-4">
                    <label class="form-label">{ "Owner" }</label>
                    <input class="form-control" ref={ self.search_form.owner.clone() } oninput={ oninput.clone() } type="text" />
                    <div class="form-text">{ "Search listings by owner address" }</div>
                </div>
                <div class="form-group col-md-4">
                    <label class="form-label">{ "Name" }</label>
                    <input class="form-control" ref={ self.search_form.title.clone() } { oninput } type="text" />
                </div>
                <div class="form-group col-md-4">
                    <label class="form-label">{ "Asset type" }</label>
                    <select class="form-select" ref={ self.search_form.collection.clone() } { onchange }>
                        <option value="" selected=true>{ "All" }</option>
                        <option value="genopets_augments">{ "Augment" }</option>
                        <option value="genopets_cosmetics">{ "Cosmetic" }</option>
                        <option value="genopets_genotype_crystals">{ "Crystal" }</option>
                        <option value="genopets_power_ups">{ "Power up" }</option>
                        <option value="genopets_reagents">{ "Reagent" }</option>
                        <option value="genopets_recipe_hunt">{ "Recipe hunt missing page" }</option>
                        <option value="genopets_terraform_seeds_sft">{ "Terraform seed" }</option>
                    </select>
                </div>
            </div>
            { pending_collect }
            <table class="table table-striped table-bordered">
                <thead>
                    <tr>
                        <th>{ "Item" }</th>
                        <th>{ "Production cost" }</th>
                        <th>{ "Orders" }</th>
                        <th>{ "Latest trades (30d)" }</th>
                    </tr>
                </thead>
                <tbody>
                    { for markets }
                </tbody>
            </table>
            <Pagination ..pagination_props />
        </div>)
    }
}

async fn token_prices(cb_token_prices: Callback<(Decimal, Decimal)>) {
    let res: JupiterPrice = Request::get("https://price.jup.ag/v3/price?ids=kiGenopAScF8VF31Zbtx2Hg8qA5ArGqvnVtXb83sotc&vsToken=So11111111111111111111111111111111111111112")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let ki_price = res
        .data
        .get("kiGenopAScF8VF31Zbtx2Hg8qA5ArGqvnVtXb83sotc")
        .unwrap()
        .price;

    let res: JupiterPrice = Request::get("https://price.jup.ag/v3/price?ids=GENEtH5amGSi8kHAtQoezp1XEXwZJ8vcuePYnXdKrMYz&vsToken=So11111111111111111111111111111111111111112")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let gene_price = res
        .data
        .get("GENEtH5amGSi8kHAtQoezp1XEXwZJ8vcuePYnXdKrMYz")
        .unwrap()
        .price;

    cb_token_prices.emit((ki_price, gene_price));
}

async fn sync_accounts(
    accounts: Vec<String>,
    cb_accounts: Callback<HashMap<Pubkey, UserAccountHeader>>,
) {
    let mut results = HashMap::new();

    for chunk in accounts.chunks(100) {
        let addresses = chunk.to_vec();

        let body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getMultipleAccounts",
            "params": [
                addresses,
                { "encoding": "jsonParsed" }
            ]
        });

        let res = Request::post("https://try-rpc.mainnet.solana.blockdaemon.tech/")
            .json(&body)
            .unwrap()
            .send()
            .await
            .unwrap()
            .json::<JsonRpcResult<Vec<Option<UiAccount>>>>()
            .await
            .unwrap();

        let iter = chunk
            .iter()
            .zip(res.result.value)
            .filter_map(|(_, account)| {
                let mut account = account?;
                let account = UserAccount::from_buffer(&mut account.data).ok()?;

                Some((account.header.market.clone(), account.header.to_owned()))
            });

        results.extend(iter);
    }

    for header in results.values() {
        console_log!("{}", header.market);
    }

    console_log!("Found {}/{} user accounts", results.len(), accounts.len());

    cb_accounts.emit(results);
}

async fn sync_markets(
    markets: Vec<MagicEdenItem>,
    cb_orders: Callback<HashMap<String, Vec<Listing>>>,
) {
    let mut results = HashMap::new();

    for chunk in markets.chunks(100) {
        let addresses = chunk
            .iter()
            .map(|item| item.asks_address.clone())
            .collect::<Vec<_>>();
        let body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getMultipleAccounts",
            "params": [
                addresses,
                { "encoding": "jsonParsed" }
            ]
        });

        let res = Request::post("https://try-rpc.mainnet.solana.blockdaemon.tech/")
            .json(&body)
            .unwrap()
            .send()
            .await
            .unwrap()
            .json::<JsonRpcResult<Vec<UiAccount>>>()
            .await
            .unwrap();

        let iter = chunk
            .iter()
            .zip(res.result.value)
            .map(|(item, mut account)| {
                let listings = Listings::from_buffer(&mut account.data).to_vec();

                (item.token_address.clone(), listings)
            });

        results.extend(iter);
    }

    cb_orders.emit(results);
}

async fn fetch_trades(cb_trades: Callback<HashMap<String, Vec<Trade>>>) {
    let trades: Vec<SftTrades> = Request::get("https://node-api.flipsidecrypto.com/api/v2/queries/b76d9ca9-cc22-48d8-9917-6760c1ec5a50/data/latest")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let trades = trades
        .into_iter()
        .map(|item| (item.base_vault, item.trades))
        .collect::<HashMap<_, _>>();

    cb_trades.emit(trades);
}

#[derive(Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
struct SftTrades {
    base_vault: String,
    trades: Vec<Trade>,
}

#[derive(Clone, Deserialize, PartialEq)]
pub struct Trade {
    ts: String,
    amount: Decimal,
    price: Decimal,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MagicEdenItem {
    base_vault_address: String,
    asks_address: String,
    #[serde(deserialize_with = "parse_base58_pubkey")]
    market_address: Pubkey,
    token_address: String,
    token_image: String,
    token_title: String,
    collection: String,
}

#[derive(Deserialize, Debug)]
struct JsonRpcResult<T> {
    // id: u64,
    // jsonrpc: String,
    result: JsonRpcResultBody<T>,
}

#[derive(Deserialize, Debug)]
struct JsonRpcResultBody<T> {
    // context: JsonRpcContext,
    value: T,
}

// #[derive(Deserialize, Debug)]
// #[serde(rename_all = "camelCase")]
// struct JsonRpcContext {
//     api_version: String,
//     slot: u64,
// }

#[derive(Deserialize, Debug)]
struct UiAccount {
    lamports: u64,
    #[serde(deserialize_with = "parse_account_data")]
    data: Vec<u8>,
    #[serde(deserialize_with = "parse_base58_pubkey")]
    owner: Pubkey,
    executable: bool,
    #[serde(default)]
    rent_epoch: u64,
}

impl From<UiAccount> for Account {
    fn from(data: UiAccount) -> Self {
        Account {
            lamports: data.lamports,
            data: data.data,
            owner: data.owner,
            executable: data.executable,
            rent_epoch: data.rent_epoch,
        }
    }
}

fn parse_account_data<'de, D: de::Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
    let data: Vec<String> = Deserialize::deserialize(deserializer)?;

    if data.len() != 2 {
        return Err(de::Error::custom("Invalid array length"));
    }

    match data[1].as_str() {
        "base64" => base64::decode(&data[0]).map_err(|e| de::Error::custom(e.to_string())),
        _ => Err(de::Error::custom(format!(
            "Unsupported encoding: {}",
            data[1]
        ))),
    }
}

fn parse_base58_pubkey<'de, D: de::Deserializer<'de>>(deserializer: D) -> Result<Pubkey, D::Error> {
    let val: String = Deserialize::deserialize(deserializer)?;

    Pubkey::from_str(&val).map_err(|e| de::Error::custom(e.to_string()))
}

#[derive(Deserialize)]
struct JupiterPrice {
    data: HashMap<String, JupiterPriceData>,
}

#[derive(Deserialize)]
struct JupiterPriceData {
    price: Decimal,
}
