#[macro_use]
extern crate lazy_static;

use std::vec;
use std::collections::BTreeMap;
use rand::distributions::{Distribution, WeightedIndex};
use rand::{Rng, rngs::StdRng, SeedableRng};

const ACTORS_IN_RANGE: usize = 3; 
const PROBABILITY_STEP: f64  = 1.0 / (ACTORS_IN_RANGE - 1) as f64;
const GAME_SEED: [u8; 32] = [3; 32];
const GAME_FRAMES: u32 = 10;

fn gather_taxes<'a>(mut actor_resources: Resources<'a>, mut tile_resources: Resources<'a>, rng: &mut StdRng) -> Option<(Resources<'a>, Resources<'a>)> {
    if rng.gen_bool(1.0) { 
        *actor_resources.entry("gold").or_insert(0) += 1;
    }
    Some((actor_resources, tile_resources))
}

fn mine_ore<'a>(mut actor_resources: Resources<'a>, mut tile_resources: Resources<'a>, rng: &mut StdRng) -> Option<(Resources<'a>, Resources<'a>)> {
    let resource_change: u32 = 1;
    let tile_resource_value = *tile_resources.entry("ore").or_insert(0);
    if rng.gen_bool(1.0) { 
        if possble_to_subtract(tile_resource_value, resource_change) {
            *actor_resources.entry("ore").or_insert(0) += resource_change;
            *tile_resources.entry("ore").or_insert(0) -= resource_change;
            return Some((actor_resources, tile_resources));
        } else {
            println!("Attempted to mine ore, but the tile is exhausted!\n");
            return None;
        }
    }
    None
}

fn mine_gem<'a>(mut actor_resources: Resources<'a>, mut tile_resources: Resources<'a>, rng: &mut StdRng) -> Option<(Resources<'a>, Resources<'a>)> {
    let resource_change: u32 = 1;
    let tile_resource_value = *tile_resources.entry("gem").or_insert(0);
    if rng.gen_bool(0.5) { 
        if possble_to_subtract(tile_resource_value, resource_change) {
            *actor_resources.entry("gem").or_insert(0) += resource_change;
            *tile_resources.entry("gem").or_insert(0) -= resource_change;
            return Some((actor_resources, tile_resources));
        } else {
            println!("Attempted to mine gem, but the tile is exhausted!\n");
            return None;
        }
    }
    None
}


const BEHAVIOURS_ARR_LENGTH: usize = 3;
lazy_static! {
    static ref BEHAVIOURS: [Behaviour<'static>; BEHAVIOURS_ARR_LENGTH] = [
        Behaviour::new(mine_ore, "mine_ore"),
        Behaviour::new(gather_taxes, "gather_taxes"),
        Behaviour::new(mine_gem, "mine_gem"),
        ];
    }
    
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

#[derive(Debug, Clone)]
struct Behaviour<'a> {
    function: fn(Resources<'a>, Resources<'a>, &mut StdRng) -> Option<(Resources<'a>, Resources<'a>)>,
    name: &'static str,
}

#[derive(Debug, Clone)]
struct BehaviourProb<'a> {
    behaviour: Behaviour<'a>,
    probability: f64,
}

#[derive(Debug, Clone)]
struct Actor<'a> {
    behaviours: Vec<BehaviourProb<'a>>,
    resources: Resources<'a>,
} 

#[derive(Debug, Default)]
struct Tile<'a> {
    actors: Vec<Actor<'a>>, 
    resources: Resources<'a>
}

impl<'a> Behaviour<'a> {
    fn new(function: fn(Resources<'a>, Resources<'a>, &mut StdRng) -> Option<(Resources<'a>, Resources<'a>)>, name:&'static str) -> Self {
        Behaviour {function, name}
    }
}

impl<'a> BehaviourProb<'a> {
    fn new(behaviour: Behaviour<'a>, probability: f64) -> Self {
        BehaviourProb {behaviour, probability}
    }
}

impl<'a> Actor<'a> {
    fn new(resources: Resources<'a>, behaviours: Vec<BehaviourProb<'a>>) -> Actor<'a> {
        Actor {resources, behaviours}
    }

    fn get_resource(&self, resource_name: &str) -> u32 {
       *self.resources.get(resource_name).unwrap_or(&0)
    }

    fn get_utility(&self) -> f64 {
        let mut total_utility = 0.0;
        for (name, &amount) in &self.resources {
            let resource_weight = RESOURCE_WEIGHTS.get(name).unwrap_or(&1.0);
            if amount > 0 {
                total_utility += resource_weight * (f64::ln(amount as f64) + 1.0);
                    // We add constant to the {log of resource amount} because without it resource change from 0 to 1 will not change utility.
                    // This is so because ln(1.0) == 0.0.
            }
        }
        total_utility
    }

    fn get_pretty_behaviours (&self) -> String {
        let behaviour_names: Vec<String> = self.behaviours.iter().map(|b| format!("{} ({:.1})", b.behaviour.name, b.probability)).collect();
        let behaviours_str = behaviour_names.join(", ");
        return behaviours_str
    }
    
}

impl<'a> Tile<'a> {
    fn new(actors: Vec<Actor<'a>>, resources: Resources<'a>) -> Tile<'a> {
        Tile{actors, resources}
    }

    fn get_resource(&self, resource_name: &str) -> u32 {
        *self.resources.get(resource_name).unwrap_or(&0)
     }

     fn update_resources(&mut self, actor_id: usize, actor_changes: Resources<'a>, tile_changes: Resources<'a>) {
        let mut actor_changes_for_printing = Vec::new();
        let mut tile_changes_for_printing = Vec::new();
    
        for (&resource_name, &new_amount) in actor_changes.iter() {
            let actor = self.actors.get_mut(actor_id).unwrap();
            let old_amount = actor.get_resource(resource_name);
            if new_amount != old_amount {
                actor.resources.insert(resource_name, new_amount);
                actor_changes_for_printing.push(format!("{}: {} -> {}", resource_name, old_amount, new_amount));
            }
        }
    
        for (&resource_name, &new_amount) in tile_changes.iter() {
            let old_amount = self.get_resource(resource_name);
            if new_amount != old_amount {
                self.resources.insert(resource_name, new_amount);
                tile_changes_for_printing.push(format!("{}: {} -> {}", resource_name, old_amount, new_amount));
            }
        }
    
        if !actor_changes_for_printing.is_empty() || !tile_changes_for_printing.is_empty() {
            let actor = self.actors.get(actor_id).unwrap();
            
            println!("Actor ID: {}\nActor Behaviours: [{}]", actor_id, actor.get_pretty_behaviours());
            println!("Resource Changes for Actor: {}", actor_changes_for_printing.join(", "));
            println!("Resource Changes for Tile: {}", tile_changes_for_printing.join(", "));
            println!("Actor's new utility: {}\n", actor.get_utility());
        }
    }


    fn execute_actions(&mut self, rng: &mut StdRng) {
        for (actor_id, actor) in self.actors.clone().iter().enumerate() {
            let probabilities: Vec<f64> = actor.behaviours.iter().map(|b| b.probability).collect();
            let weighted_distribution = WeightedIndex::new(&probabilities).unwrap();
            let chosen_index = weighted_distribution.sample(rng);
            let chosen_behaviour = actor.behaviours[chosen_index].behaviour.function;

            // First-come, first-served resource extraction system:
            // If the resource change is possible (thus behaviour is also possible) for the actor we are currently iterating over, the change will occur.
            // Consequently, other actors may fail in attempting to execute exactly the same behavior in the same "frame" due to a lack of resources in the Tile.
            if let Some((new_actor_resources, new_tile_resources)) = chosen_behaviour(actor.resources.clone(), self.resources.clone(), rng) {
                self.update_resources(actor_id, new_actor_resources, new_tile_resources);
            }
        }
    }

    fn get_highest_utility_actor(&self) -> Option<&Actor<'a>> {
        self.actors
            .iter()
            .max_by(|a, b| a.get_utility().partial_cmp(&b.get_utility()).unwrap_or(std::cmp::Ordering::Equal))
    }
}

fn possble_to_subtract(value: u32, amount_to_sustract: u32) -> bool {
    if amount_to_sustract > value {
        false
    } else {
        true
    }
}

fn generate_probability_distributions() -> Vec<Vec<f64>> {
    let mut probabilities_for_all_actors = Vec::new();
    probability_distributions_recursion(
        &mut probabilities_for_all_actors,
        &mut Vec::new(),
        ACTORS_IN_RANGE - 1,
        BEHAVIOURS_ARR_LENGTH - 1
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
        // println!("{:?}", probabilities_for_storage);
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
    let mut tile_resources: Resources = BTreeMap::new();
    tile_resources.insert("ore", 50);
    tile_resources.insert("gem", 50);

    let mut tile = Tile::new(vec![], tile_resources);
    for probs in probabilities_for_actors.iter() {
        let behaviour_probs: Vec<BehaviourProb> = BEHAVIOURS
        .iter()
        .zip(probs.iter())
        .map(|(b, &p)| BehaviourProb::new(b.clone(), p))
        .collect();
        tile.actors.push(Actor::new(BTreeMap::new(), behaviour_probs));
    }
    
    let mut rng = StdRng::from_seed(GAME_SEED);
    for _ in 1..GAME_FRAMES {
        tile.execute_actions(&mut rng);
    }

    let winner = tile.get_highest_utility_actor().unwrap();
    println!("Actor's with such behaviours won: [{:?}]\nActor utility is {:?}\n", winner.get_pretty_behaviours(), winner.get_utility())

}
