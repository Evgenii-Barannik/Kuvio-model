#[macro_use]
extern crate lazy_static;

use std::fs;
// use rayon::ThreadPoolBuilder;
use rayon::prelude::*;
use std::time::Instant;
use std::vec;
use std::collections::BTreeMap;
use itertools::Itertools;
use rand::distributions::{Distribution, WeightedIndex};
use rand::{Rng, rngs::StdRng, SeedableRng};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::fs::write;

#[derive(Debug, Clone, Hash)]
enum HyperParam {
    NumOfProbSteps(usize),
    GameTicks(usize),
    GameSeed([u8; 32])
}

// Hyperparameter ranges that define searched phasespace
lazy_static! {
    static ref HYPER_PARAMS_RANGES: Vec<Vec<HyperParam>> = vec![
        (1..=3).map(HyperParam::NumOfProbSteps).collect::<Vec<_>>(),
        (100..=100).map(HyperParam::GameTicks).collect::<Vec<_>>(),
        vec![HyperParam::GameSeed([2; 32])] 
    ];
}

lazy_static! {
    static ref BEHAVIOURS: [Behaviour<'static>; 4] = [
        Behaviour::new(gather_taxes, "gather_taxes"),
        Behaviour::new(mine_ore, "mine_ore"),
        Behaviour::new(mine_gem, "mine_gem"),
        Behaviour::new(smart_behaviour, "smart_behaviour"),
    ];
}

lazy_static! {
    static ref TILE_INITIAL_RESOURCES: BTreeMap<&'static str, u32> = {
        let mut tile_resources: Resources = BTreeMap::new();
        tile_resources.insert("ore", 1000);
        tile_resources.insert("gem", 200);

        tile_resources
    };
}

lazy_static! {
    static ref RESOURCE_WEIGHTS: BTreeMap<&'static str, f64> = {
        let mut weights = BTreeMap::new();
        weights.insert("gold", 1.0);
        weights.insert("wood", 1.0);
        weights.insert("ore", 1.0);
        weights.insert("mercury", 5.0);
        weights.insert("sulfur", 5.0);
        weights.insert("crystal", 5.0);
        weights.insert("gem", 5.0);
        weights
    };
}

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
            // println!("Attempted to mine ore, but the tile is exhausted!\n");
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
            // println!("Attempted to mine gem, but the tile is exhausted!\n");
            return None;
        }
    }
    None
}

fn smart_behaviour<'a>(mut actor_resources: Resources<'a>, mut tile_resources: Resources<'a>, rng: &mut StdRng) -> Option<(Resources<'a>, Resources<'a>)> {
    if *tile_resources.entry("gem").or_insert(0) > 0 {
        return mine_gem(actor_resources, tile_resources, rng)
    } else {
        return gather_taxes(actor_resources, tile_resources, rng)
    }
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
        let behaviour_names: Vec<String> = self.behaviours.iter().map(|b| format!("{} ({:.0}%)", b.behaviour.name, b.probability * 100.0)).collect();
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

     fn update_resources(&mut self, actor_id: usize, actor_changes: Resources<'a>, tile_changes: Resources<'a>, log: &mut String) {
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


            log.push_str(&format!("Actor ID: {}\nActor Behaviours: [{}]\n", actor_id, actor.get_pretty_behaviours()));
            log.push_str(&format!("Resource Changes for Actor: {}\n", actor_changes_for_printing.join(", ")));
            log.push_str(&format!("Resource Changes for Tile: {}\n", tile_changes_for_printing.join(", ")));
            log.push_str(&format!("Actor's new utility: {}\n\n", actor.get_utility()));
        }
    }


    fn execute_actions(&mut self, rng: &mut StdRng, log: &mut String) {
        for (actor_id, actor) in self.actors.clone().iter().enumerate() {
            let probabilities: Vec<f64> = actor.behaviours.iter().map(|b| b.probability).collect();
            let weighted_distribution = WeightedIndex::new(&probabilities).unwrap();
            let chosen_index = weighted_distribution.sample(rng);
            let chosen_behaviour = actor.behaviours[chosen_index].behaviour.function;

            // First-come, first-served resource extraction system:
            // If the resource change is possible (thus behaviour is also possible) for the actor we are currently iterating over, the change will occur.
            // Consequently, other actors may fail in attempting to execute exactly the same behavior in the same "frame" due to a lack of resources in the Tile.
            if let Some((new_actor_resources, new_tile_resources)) = chosen_behaviour(actor.resources.clone(), self.resources.clone(), rng) {
                self.update_resources(actor_id, new_actor_resources, new_tile_resources, log);
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

fn generate_probability_distributions(actors_in_crossection: usize, log: &mut String) -> Vec<Vec<f64>> {
    match actors_in_crossection {
        0 => {panic!("There should be at least one actor.")},
        1 => {
            let len = BEHAVIOURS.len();
            let probabilities_for_actor = vec![vec![1.0/(len as f64); len]];
            log.push_str(&format!("Probability distribution for actor:\n{:?}\n\n", probabilities_for_actor));
            probabilities_for_actor
        }, 
        _ => {
            let mut probabilities_for_all_actors = Vec::new();

            probability_distributions_recursion(
                &mut probabilities_for_all_actors,
                &mut Vec::new(),
                actors_in_crossection - 1,
                BEHAVIOURS.len() - 1,
                actors_in_crossection,
            );
            
            log.push_str("Probability distributions for actors:\n");
                for distribution in probabilities_for_all_actors.iter() {
                log.push_str(&format!("{:?}\n", distribution));
            }
            log.push_str("\n");

            probabilities_for_all_actors
        }
    }
}

fn probability_distributions_recursion(
    probabilities_for_all_actors: &mut Vec<Vec<f64>>,
    probabilities_for_actor: &mut Vec<f64>,
    remaining_probability_steps: usize,
    remaining_recursion_depth: usize,
    actors_in_crossection: usize,
) {
    let probability_step: f64 = 1.0 / (actors_in_crossection - 1) as f64;
    if remaining_recursion_depth == 0 {
        let mut probabilities_for_storage = probabilities_for_actor.clone();
        probabilities_for_storage.push(remaining_probability_steps as f64 * probability_step);
        // println!("{:?}", probabilities_for_storage);
        probabilities_for_all_actors.push(probabilities_for_storage);
    } else {
        for i in 0..=remaining_probability_steps {
            let mut probabilities_for_recursion = probabilities_for_actor.clone();
            probabilities_for_recursion.push(i as f64 * probability_step);
            probability_distributions_recursion(
                probabilities_for_all_actors, 
                &mut probabilities_for_recursion, 
                remaining_probability_steps - i, 
                remaining_recursion_depth - 1,
                actors_in_crossection,
            );
        }
    }
}

fn hash_hyper_params(hyper_params: &[HyperParam]) -> u64 {
    let mut hasher = DefaultHasher::new();
    hyper_params.hash(&mut hasher);
    hasher.finish()
}

fn main() {
    let timer = Instant::now();
    fs::create_dir_all("output").unwrap();
    // rayon::ThreadPoolBuilder::new().num_threads(1).build_global().unwrap();

    HYPER_PARAMS_RANGES.clone()
    .into_iter()
    .multi_cartesian_product()
    .collect::<Vec<_>>()
    .into_par_iter()
    .for_each( |hyper_params| {

        if let [HyperParam::NumOfProbSteps(actors_in_crossection),
                HyperParam::GameTicks(game_ticks),
                HyperParam::GameSeed(game_seed)] = hyper_params[..] {

                let mut log = String::new();
                log.push_str(&format!("Number of possible probability values for one behaviour: {},\nTotal game ticks: {},\nGame seed: {:?}\n\n",
                actors_in_crossection, game_ticks, game_seed));

                let probabilities_for_actors = generate_probability_distributions(actors_in_crossection, &mut log);
                let mut tile = Tile::new(vec![], TILE_INITIAL_RESOURCES.clone());
                for probs in probabilities_for_actors.iter() {
                    let behaviour_probs: Vec<BehaviourProb> = BEHAVIOURS
                    .iter()
                    .zip(probs.iter())
                    .map(|(b, &p)| BehaviourProb::new(b.clone(), p))
                    .collect();
                    tile.actors.push(Actor::new(BTreeMap::new(), behaviour_probs));
                }
                
                let mut rng = StdRng::from_seed(game_seed);
                for t in 0..game_ticks {
                    log.push_str(&format! ("-- Game tick {} --\n", t));
                    tile.execute_actions(&mut rng, &mut log);
                }
                
            let winner = tile.get_highest_utility_actor().unwrap();

            log.push_str(&format!("Actor with such behaviours won: [{:?}]\nActor's resources are: {:?}\nActor's utility is: {:?}\n",
            winner.get_pretty_behaviours(),
            winner.resources,
            winner.get_utility()));

            let hash = hash_hyper_params(&hyper_params);
            let file_name = format!("output/{}.txt", hash);
            write(&file_name, log).unwrap();

        } else { panic!("Hyperparameters were not parsed correctly.") }
    });

    println!("Execution time: {:?}", timer.elapsed());
}
