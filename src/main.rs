use asset_agnostic_orderbook::state::{
    critbit::{LeafNode, Node, Slab},
    AccountTag,
};
use dex_v4::state::CallBackInfo;
use gloo_net::http::Request;
use rust_decimal::{Decimal, MathematicalOps};
use serde::{de, Deserialize};
use serde_json::json;
use solana_sdk::{account::Account, pubkey, pubkey::Pubkey};
use std::{collections::HashMap, str::FromStr};
use web_sys::HtmlInputElement;
use yew::prelude::*;

#[allow(unused)]
macro_rules! console_log {
    ($($t:tt)*) => (web_sys::console::log_1(&format_args!($($t)*).to_string().into()))
}

const SERUM_V4: Pubkey = pubkey!("srmv4uTCPF81hWDaPyEN2mLZ8XbvzuEM6LsAxR8NpjU");

// Workaround to use the macro in other modules. Kudos to
// https://stackoverflow.com/questions/26731243/how-do-i-use-a-macro-across-module-files
pub(crate) use console_log;

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
    orders: HashMap<String, Vec<(Pubkey, LeafNode)>>,
    markets: Vec<MagicEdenItem>,
    search_data: SearchFormData,
    search_form: SearchForm,
    page: usize,
}

pub enum AppMsg {
    Orders(HashMap<String, Vec<(Pubkey, LeafNode)>>),
    Search(SearchFormData),
    Page(usize),
}

impl From<&SearchForm> for AppMsg {
    fn from(search_form: &SearchForm) -> Self {
        fn get_val(node_ref: &NodeRef) -> String {
            node_ref.cast::<HtmlInputElement>().unwrap().value()
        }

        let owner = get_val(&search_form.owner);

        let data = SearchFormData {
            title: get_val(&search_form.title),
            owner: Pubkey::from_str(&owner).ok(),
            collection: get_val(&search_form.collection),
        };

        AppMsg::Search(data)
    }
}

impl Component for App {
    type Message = AppMsg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let cb = ctx.link().callback(|orders| AppMsg::Orders(orders));
        let genopets_sfts = include_str!("../collections/genopets_sfts.json");
        let markets: Vec<MagicEdenItem> = serde_json::from_str(genopets_sfts).unwrap();

        wasm_bindgen_futures::spawn_local(sync_markets(markets.clone(), cb));

        Self {
            orders: HashMap::new(),
            markets,
            search_data: SearchFormData::default(),
            search_form: SearchForm::default(),
            page: 0,
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            AppMsg::Orders(orders) => self.orders = orders,
            AppMsg::Search(data) => self.search_data = data,
            AppMsg::Page(page) => self.page = page,
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
                orders.iter().find(|(owner, _)| owner == owner_key)?;
            }

            Some((item, orders, owner_key))
        });

        let count = markets.clone().count();

        let markets = markets
            .map(|(item, orders, owner_key)| {
                let orders = orders.iter().filter_map(|(owner, node)| {
                    match owner_key {
                        Some(owner_key) if &owner_key != owner => return None,
                        _ => {}
                    }

                    let price = compute_ui_price(node.price());

                    Some(html!(<tr key={ node.key }>
                        <td>{ price.round_dp(3).to_string() }</td>
                        <td>{ node.base_quantity }</td>
                    </tr>))
                });

                let asset_kind = match item.collection.as_str() {
                    "genopets_augments" => "Augment",
                    "genopets_genotype_crystals" => "Crystal",
                    "genopets_reagents" => "Reagent",
                    "genopets_cosmetics" => "Cosmetic",
                    "genopets_power_ups" => "Power up",
                    "genopets_terraform_seeds_sft" => "Terraform seed",
                    "genopets_recipe_hunt" => "Recipe hunt missing page",
                    _ => "Unknown"
                };

                html!(<tr key={ item.token_address.clone() }>
                    <td>{ asset_kind }</td>
                    <td>
                        <a href={ format!("https://magiceden.io/sft/{}", item.market_address) } target="_blank">{ &item.token_title }</a>
                    </td>
                    <td><img src={ Some(item.token_image.clone()) } style="width: 250px; height: 250px" /></td>
                    <td>
                        <div style="height: 250px; overflow: auto">
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
                        </div>
                    </td>
                </tr>)
            })
            .skip(self.page * 10)
            .take(10);

        let mut pages = count / 10;
        if count % 10 != 0 {
            pages += 1;
        }

        let pagination = (0..pages).into_iter().map(|page| {
            let onclick = ctx.link().callback(move |_| AppMsg::Page(page));
            let class = if page == self.page {
                "page-item active"
            } else {
                "page-item"
            };

            html!(<li { class }>
                    <a class="page-link" { onclick }>{ page + 1 }</a>
                </li>)
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
                        <option value="genopets_genotype_crystals">{ "Crystal" }</option>
                        <option value="genopets_reagents">{ "Reagent" }</option>
                        <option value="genopets_cosmetics">{ "Cosmetic" }</option>
                        <option value="genopets_power_ups">{ "Power up" }</option>
                        <option value="genopets_terraform_seeds_sft">{ "Terraform seed" }</option>
                        <option value="genopets_recipe_hunt">{ "Recipe hunt missing page" }</option>
                    </select>
                </div>
            </div>
            <table class="table table-striped table-bordered">
                <thead>
                    <tr>
                        <th>{ "Asset type" }</th>
                        <th>{ "Name" }</th>
                        <th>{ "Image" }</th>
                        <th>{ "Orders" }</th>
                    </tr>
                </thead>
                <tbody>
                    { for markets }
                </tbody>
            </table>
            <ul class="pagination">
                { for pagination }
            </ul>
        </div>)
    }
}

async fn sync_markets(
    markets: Vec<MagicEdenItem>,
    cb: Callback<HashMap<String, Vec<(Pubkey, LeafNode)>>>,
) {
    // TODO: make it an interval?

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
                let slab =
                    Slab::<CallBackInfo>::from_buffer(&mut account.data, AccountTag::Asks).unwrap();
                let orders = MySlabIterator::new(slab, true).collect::<Vec<_>>();

                (item.token_address.clone(), orders)
            });

        results.extend(iter);
    }

    cb.emit(results);
}

struct MySlabIterator<'a> {
    slab: Slab<'a, CallBackInfo>,
    search_stack: Vec<u32>,
    ascending: bool,
}

impl<'a> MySlabIterator<'a> {
    fn new(slab: Slab<'a, CallBackInfo>, ascending: bool) -> Self {
        Self {
            search_stack: match slab.root() {
                Some(root_node) => vec![root_node],
                None => vec![],
            },
            slab,
            ascending,
        }
    }
}

impl<'a> Iterator for MySlabIterator<'a> {
    type Item = (Pubkey, LeafNode);

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(current) = self.search_stack.pop() {
            match Node::from_handle(current) {
                Node::Inner => {
                    let n = &self.slab.inner_nodes[(!current) as usize];
                    self.search_stack.push(n.children[self.ascending as usize]);
                    self.search_stack.push(n.children[!self.ascending as usize]);
                }
                Node::Leaf => {
                    let owner = self.slab.callback_infos[current as usize].user_account;
                    let leaf = self.slab.leaf_nodes[current as usize];

                    return Some((owner, leaf));
                }
            }
        }

        None
    }
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

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MagicEdenItem {
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
