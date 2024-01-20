#[macro_use]
extern crate lazy_static;

use std::collections::BTreeMap;
use rand::distributions::{Distribution, WeightedIndex};
use rand::{Rng, rngs::StdRng, SeedableRng};

const RESOURCE_BIAS: f64 = 1.0;
const SEED: [u8; 32] = [2; 32];
lazy_static! {
    static ref RESOURCE_WEIGHTS: BTreeMap<&'static str, f64> = {
        let mut m = BTreeMap::new();
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

type Resources<'a> = BTreeMap<&'a str, u32>;
type Behaviour<'a> = fn(Resources<'a>, &mut StdRng) -> Resources<'a>;
type BehaviourProbs<'a> = Vec<(Behaviour<'a>, f64)>;

#[derive(Debug, Default)]
struct Actor<'a> {
    id: &'a str,
    resources: Resources<'a>,
    behaviours: BehaviourProbs<'a>,
} 

impl<'a> Actor<'a> {
    fn new(id: &'a str, resources: Resources<'a>, behaviours: BehaviourProbs<'a>) -> Actor<'a> {
        Actor {id, resources, behaviours}
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
            println!("Resource changes for {}:\n{}\n", self.id, updates_str);
        }
    }

    fn add_to_resource(&mut self, name: &'a str, amount_to_add: u32) {
        let old_amount = self.get_resource(name);
        let new_amount = old_amount + amount_to_add;
        let change = BTreeMap::from([(name, new_amount)]);
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
        println!("Resources for {}:\n{:?}\nUtility: {:.4}\n", &self.id, &self.resources, &self.utility());
    }

    fn execute_action(&mut self, rng: &mut StdRng) {
        let (behaviours, probabilities): (Vec<Behaviour>, Vec<f64>) = 
        self.behaviours
        .iter()
        .cloned()
        .unzip();

        let weighted_dist = WeightedIndex::new(&probabilities).unwrap();
        let chosen_behaviour = behaviours[weighted_dist.sample(rng)];
        let new_resources = chosen_behaviour(self.resources.clone(), rng);

        self.update_resources(new_resources);
    }
}


fn mine_gold<'a>(mut resources: Resources<'a>, _: &mut StdRng) -> Resources<'a> {
    *resources.entry("gold").or_insert(0) += 1;
    resources
}

fn mine_gems<'a>(mut resources: Resources<'a>, rng: &mut StdRng) -> Resources<'a> {
    if rng.gen_bool(0.5) { 
        *resources.entry("gem").or_insert(0) += 1;
    }
    resources
}

fn main() {
    let mut r = StdRng::from_seed(SEED);
    
    let alice_behaviours: Vec<(Behaviour, f64)> = vec![(mine_gold as Behaviour,1.0)]; 
    let mut alice = Actor::new("Alice", BTreeMap::new(), alice_behaviours);
    
    let bob_behaviours: Vec<(Behaviour, f64)> = vec![(mine_gold as Behaviour, 0.8), (mine_gems as Behaviour,0.2)]; 
    let mut bob = Actor::new("Bob", BTreeMap::new(), bob_behaviours);

    for _ in 1..100 {
        alice.execute_action(&mut r);
        bob.execute_action(&mut r);
    }

    alice.print_resources();
    bob.print_resources();
}
