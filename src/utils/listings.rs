use asset_agnostic_orderbook::state::{
    critbit::{Node, Slab},
    AccountTag,
};
use dex_v4::state::CallBackInfo;
use solana_sdk::pubkey::Pubkey;

pub struct Listings<'a> {
    slab: Slab<'a, CallBackInfo>,
    search_stack: Vec<u32>,
}

impl<'a> Listings<'a> {
    pub fn from_buffer(buf: &'a mut [u8]) -> Self {
        let slab = Slab::from_buffer(buf, AccountTag::Asks).unwrap();

        Self {
            search_stack: match slab.root() {
                Some(root_node) => vec![root_node],
                None => vec![],
            },
            slab,
        }
    }

    pub fn to_vec(self) -> Vec<Listing> {
        self.collect()
    }
}

impl<'a> Iterator for Listings<'a> {
    type Item = Listing;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(current) = self.search_stack.pop() {
            match Node::from_handle(current) {
                Node::Inner => {
                    let n = &self.slab.inner_nodes[(!current) as usize];
                    self.search_stack.push(n.children[1]);
                    self.search_stack.push(n.children[0]);
                }
                Node::Leaf => {
                    let leaf = self.slab.leaf_nodes[current as usize];

                    return Some(Listing {
                        key: leaf.key,
                        owner: self.slab.callback_infos[current as usize].user_account,
                        price: leaf.price(),
                        base_quantity: leaf.base_quantity,
                    });
                }
            }
        }

        None
    }
}

#[derive(Clone, PartialEq)]
pub struct Listing {
    pub key: u128,
    pub owner: Pubkey,
    pub price: u64,
    pub base_quantity: u64,
}
