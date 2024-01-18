#[macro_use]
extern crate lazy_static;

use std::collections::HashMap;
use rand::distributions::{Distribution, WeightedIndex};
use rand::thread_rng;
use rand::Rng;

const RESOURCE_BIAS: f64 = 1.0;
lazy_static! {
    static ref RESOURCE_WEIGHTS: HashMap<&'static str, f64> = {
        let mut m = HashMap::new();
        m.insert("gold", 1.0);
        m.insert("wood", 1.0);
        m.insert("ore", 1.0);
        m.insert("mercury", 5.0);
        m.insert("sulfur", 5.0);
        m.insert("crystal", 5.0);
        m.insert("gem", 5.0);
        m
    };
}

type Resources<'a> = HashMap<&'a str, u32>;
type Behaviour<'a> = fn(Resources<'a>) -> Resources<'a>; 

#[derive(Debug, Default)]
struct Actor<'a> {
    name: &'a str,
    resources: Resources<'a>,
    behaviours: HashMap<Behaviour<'a>, f64> 
} 

impl<'a> Actor<'a> {
    fn new(
        actor_name: &'a str,
        initial_resources: Resources<'a>,
        initial_behaviours: HashMap<Behaviour<'a>, f64>
    ) -> Actor<'a> { 
        Actor {
            name: actor_name,
            resources: initial_resources,
            behaviours: initial_behaviours
        }
    }

    fn get_resource(&self, resource_name: &str) -> u32 {
        *self.resources.get(resource_name).unwrap_or(&0)
    }

    fn update_resources(&mut self, changes: Resources<'a>) {
        let mut updates_for_printing = Vec::new();
        for (&resource_name, new_amount) in changes.iter() {
            let old_amount = &self.get_resource(resource_name);
            if new_amount != old_amount {
                self.resources.insert(resource_name, *new_amount);
                updates_for_printing.push(format!("\"{}\": {} -> {}", resource_name, old_amount, new_amount));
            }
        }

        if !updates_for_printing.is_empty() {
            let updates_str = updates_for_printing.join("\n");
            println!("Resource changes for {}:\n{}\n", self.name, updates_str);
        }
    }

    fn add_to_resource(&mut self, name: &'a str, amount_to_add: u32) {
        let old_amount = self.get_resource(name);
        let new_amount = old_amount + amount_to_add;
        let change = HashMap::from([(name, new_amount)]);
        self.update_resources(change);
    }

    // We add RESOURCE_BIAS because without it resource change 0 -> 1 will not change utility.
    // It is so because ln(1.0) == 0.0.
    fn utility(&self) -> f64 {
        let mut total_utility = 0.0;
        for (name, &amount) in &self.resources {
            let resource_weight = RESOURCE_WEIGHTS.get(name).unwrap_or(&1.0);
            if amount > 0 {
                total_utility += resource_weight * (f64::ln(amount as f64) + RESOURCE_BIAS);
            }
        }
        total_utility
    }
    
    fn print_resources(&self) {
        println!("Resources for {}:\n{:?}\nUtility: {:.4}\n", &self.name, &self.resources, &self.utility());
    }

    fn execute_action_randomly(&mut self) {
        let behaviours: Vec<_> = self.behaviours.keys().collect();
        let weights: Vec<_> = self.behaviours.values().cloned().collect();

        let weighted_dist = WeightedIndex::new(&weights).unwrap();
        let mut rng = thread_rng();

        let chosen_behaviour = behaviours[weighted_dist.sample(&mut rng)];
        let new_resources = chosen_behaviour(self.resources.clone());

        self.update_resources(new_resources);
    }
}


fn mine_gold(mut resources: Resources) -> Resources {
    *resources.entry("gold").or_insert(0) += 1;
    resources
}

fn mine_gems(mut resources: Resources) -> Resources {
    if rand::thread_rng().gen_bool(0.5) { 
        *resources.entry("gems").or_insert(0) += 1;
    }
    resources
}


fn main() {
    let mut gems_strategy = HashMap::new();
    gems_strategy.insert(mine_gems as Behaviour, 1.0);
    let mut alice = Actor::new("Alice", HashMap::new(), gems_strategy);

    let mut gems_and_gold_strategy = HashMap::new();
    gems_and_gold_strategy.insert(mine_gems as Behaviour, 0.8);
    gems_and_gold_strategy.insert(mine_gold as Behaviour, 0.2);
    let mut bob = Actor::new("Bob", HashMap::new(), gems_and_gold_strategy);

    for _ in 1..100 {
        alice.execute_action_randomly();
        bob.execute_action_randomly();
    }

    alice.print_resources();
    bob.print_resources();
}
