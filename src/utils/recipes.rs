use crate::MagicEdenItem;
use rust_decimal::Decimal;
use std::collections::HashMap;
pub(crate) struct Recipes {
    recipes: HashMap<String, HashMap<String, Decimal>>,
    markets: Vec<MagicEdenItem>,
}

impl Recipes {
    pub fn new(markets: Vec<MagicEdenItem>) -> Recipes {
        let genopets_recipes = include_str!("../../collections/genopets_recipes.json");

        Self {
            markets,
            recipes: serde_json::from_str(genopets_recipes).unwrap(),
        }
    }

    pub fn get(&self, token_address: &str) -> Option<Recipe> {
        let mut recipe = Recipe::default();

        match token_address {
            "EaRtHRxHp1ftdfnJFds9UrCDNaSGxhdnRUucevNr1DzA"
            | "FireKR7LgjyzjsLnxaNZwa7dnJncDSidD4cXGhTGz2eU"
            | "Meta1cQ29N8S4cSwJScHZYtXV6J5Cy55oEA8vRVhh8K"
            | "WATErpZ2ZBjgAxyttoEjckuTuCe9pEckSabCeENLTYq"
            | "woodN5KSiHEAhaCrZVh3vScGta7u6r5Vp3UbqDFuD4e" => {
                // Crystals have a known cost of 10 KI
                recipe.ki_cost = Decimal::TEN;

                return Some(recipe);
            }
            _ => {}
        }

        for (reagent, amount) in self.recipes.get(token_address)? {
            match reagent.as_str() {
                "kiGenopAScF8VF31Zbtx2Hg8qA5ArGqvnVtXb83sotc" => {
                    recipe.reagents.push((String::from("KI"), *amount));
                    recipe.ki_cost += amount;
                }
                "GENE" => {
                    recipe.reagents.push((String::from("(s)GENE"), *amount));
                    recipe.gene_cost += amount;
                }
                _ => {
                    let market = self
                        .markets
                        .iter()
                        .find(|item| &item.token_address == reagent)?;

                    match market.collection.as_str() {
                        "genopets_genotype_crystals" => {
                            recipe.ki_cost += Decimal::TEN * amount;
                        }
                        _ => {}
                    }

                    if let Some(sub_recipe) = self.get(reagent) {
                        recipe.ki_cost += sub_recipe.ki_cost * amount;
                        recipe.gene_cost += sub_recipe.gene_cost * amount;
                    }

                    recipe.reagents.push((market.token_title.clone(), *amount));
                }
            }
        }

        Some(recipe)
    }
}

#[derive(Default)]
pub struct Recipe {
    pub ki_cost: Decimal,
    pub gene_cost: Decimal,
    pub reagents: Vec<(String, Decimal)>,
}
