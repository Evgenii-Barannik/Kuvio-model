#[macro_use]
extern crate lazy_static;

use std::vec;
use std::collections::BTreeMap;
use rand::distributions::{Distribution, WeightedIndex};
use rand::{Rng, rngs::StdRng, SeedableRng};

const BEHAVIOUR_ARR_LENGTH: usize = 3;

const ACTORS_IN_SPACE: usize = 11; 
const PROBABILITY_STEP: f64  = 1.0 / (ACTORS_IN_SPACE - 1) as f64;

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

#[derive(Debug, Default)]
struct Actor<'a> {
    resources: Resources<'a>,
    behaviour_probs: Vec<(&'a NamedBehaviour<'a>, f64)>,
} 

impl<'a> Actor<'a> {
    fn new(resources: Resources<'a>, behaviour_probs: Vec<(&'a NamedBehaviour<'a>, f64)>) -> Actor<'a> {
        Actor {resources, behaviour_probs}
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
            let updates_str = updates_for_printing.join(", ");
            let behaviour_names: Vec<String> = self.behaviour_probs
                .iter()
                .map(|(behaviour, prob)| format!("({}: {:.1})", behaviour.name, prob))
                .collect();
            let behaviours_str = behaviour_names.join(", ");
            println!("Resource changes for actor with behaviours\n[{}]:\n{}\n", behaviours_str, updates_str);
        }
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

}

struct Pallet<'a> {
    actors: Vec<Actor<'a>>
}

impl<'a> Pallet<'a> {
    fn new(actors: Vec<Actor<'a>>) -> Pallet<'a> {
        Pallet{actors}
    }

    fn execute_actions_for_actors(&mut self, rng:&mut StdRng) {
        for actor in &mut self.actors {
            let (behaviours, probabilities): (Vec<&NamedBehaviour>, Vec<f64>) = 
            actor.behaviour_probs.iter().cloned().unzip();

            let weighted_dist = WeightedIndex::new(&probabilities).unwrap();
            let chosen_behaviour = (*behaviours[weighted_dist.sample(rng)]).func;
            let new_resources = chosen_behaviour(actor.resources.clone(), rng);
            actor.update_resources(new_resources);
        }
    }

    fn actor_with_highest_utility(&self) -> Option<&Actor<'a>> {
        self.actors
            .iter()
            .max_by(|a, b| a.utility().partial_cmp(&b.utility()).unwrap_or(std::cmp::Ordering::Equal))
    }
}



fn mine_gold<'a>(mut resources: Resources<'a>, _: &mut StdRng) -> Resources<'a> {
    *resources.entry("ore").or_insert(0) += 1;
    resources
}

fn mine_ore<'a>(mut resources: Resources<'a>, rng: &mut StdRng) -> Resources<'a> {
    if rng.gen_bool(0.9) { 
        *resources.entry("gold").or_insert(0) += 1;
    }
    resources
}

fn mine_gem<'a>(mut resources: Resources<'a>, rng: &mut StdRng) -> Resources<'a> {
    if rng.gen_bool(0.5) { 
        *resources.entry("gem").or_insert(0) += 1;
    }
    resources
}

#[derive(Debug)]
struct NamedBehaviour<'a> {
    func: Behaviour<'a>,
    name: &'static str,
}

impl<'a> NamedBehaviour<'a> {
    fn new(func: Behaviour<'a>, name: &'static str) -> Self {
        NamedBehaviour { func, name }
    }
}


fn generate_probability_distributions() -> Vec<Vec<f64>> {
    let mut probabilities_for_all_actors = Vec::new();
    probability_distributions_recursion(
        &mut probabilities_for_all_actors,
        &mut Vec::new(),
        ACTORS_IN_SPACE - 1,
        BEHAVIOUR_ARR_LENGTH - 1
    );
    probabilities_for_all_actors
}

fn probability_distributions_recursion(
    probabilities_for_all_actors: &mut Vec<Vec<f64>>,
    probabilities_for_actor: &mut Vec<f64>,
    remaining_probability_steps: usize,
    remaining_recursion_depth: usize,
) {
    if remaining_recursion_depth == 0 {
        let mut probabilities_for_storage = probabilities_for_actor.clone();
        probabilities_for_storage.push(remaining_probability_steps as f64 * PROBABILITY_STEP);
        println!("{:?}", probabilities_for_storage);
        probabilities_for_all_actors.push(probabilities_for_storage);
    } else {
        for i in 0..=remaining_probability_steps {
            let mut probabilities_for_recursion = probabilities_for_actor.clone();
            probabilities_for_recursion.push(i as f64 * PROBABILITY_STEP);
            probability_distributions_recursion(
                probabilities_for_all_actors, 
                &mut probabilities_for_recursion, 
                remaining_probability_steps - i, 
                remaining_recursion_depth - 1
            );
        }
    }
}

fn main() {
    let probabilities_for_actors = generate_probability_distributions();
    let actions = vec![
        NamedBehaviour::new(mine_ore as Behaviour, "mine_ore"),
        NamedBehaviour::new(mine_gold as Behaviour, "mine_gold"),
        NamedBehaviour::new(mine_gem as Behaviour, "mine_gem"),
    ];

    
    let mut pallet = Pallet::new(vec![]);
    for probs in probabilities_for_actors.iter() {
        pallet.actors.push(
            Actor::new(
                BTreeMap::new(),
                actions.iter().zip(probs.iter().cloned()).collect()
            )
        )
    }
    
    let mut rng = StdRng::from_seed(SEED);
    for _ in 1..1000 {
        pallet.execute_actions_for_actors(&mut rng);
    }

    let winner = pallet.actor_with_highest_utility().unwrap();
    println!("Actor with highest utility {} is\n{:?}", winner.utility(), winner)

}
