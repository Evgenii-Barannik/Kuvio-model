use std::fs;
use std::time::Instant;
use std::vec;
use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::fs::write;
use itertools::Itertools;
use itertools::iproduct;
use rand::distributions::{Distribution, WeightedIndex};
use rand::{Rng, rngs::StdRng, SeedableRng};
use rayon::prelude::*;
use rayon::ThreadPoolBuilder;
use lazy_static::lazy_static;

#[derive(Debug, Clone, Hash)]
enum HyperParam {
    NumOfProbValues(usize),
    GameTicks(usize),
    GameSeed([u8; 32]),
    InitialTileResources(Resources),
}

// Hyperparameter ranges that define region of phasespace that we explore
lazy_static! {
    static ref HYPER_PARAMS_RANGES: Vec<Vec<HyperParam>> = vec![
        (5..=6).map(HyperParam::NumOfProbValues).collect::<Vec<_>>(),
        [4000, 5000].into_iter().map(HyperParam::GameTicks).collect::<Vec<_>>(),
        vec![HyperParam::GameSeed([2; 32])],
        INITIAL_RESOURCE_COMBINATIONS.to_vec(),
    ];
}

lazy_static! {
    static ref INITIAL_RESOURCE_COMBINATIONS: Vec<HyperParam> = {
        let gold_range = (1u32..=4u32).map(|x| 500 * x); 
        let wood_range = (1u32..=4u32).map(|x| 1000 * x); 
        
        iproduct!(gold_range, wood_range)
        .map(|(gold, wood)| {
            HyperParam::InitialTileResources(BTreeMap::from([(Resource::Gold, gold), (Resource::Wood, wood)]))})
        .collect::<Vec<_>>()
    };
}
        
#[derive(PartialEq, PartialOrd, Eq, Ord, Debug, Hash, Clone)]
enum Resource {
    Gold,
    Wood,
    Reputation,
}

type BehaviourFn = fn(usize, &mut Tile, &mut StdRng) -> Result<(), String>; // TODO Change usize to Actor reference if Rust memory system will allow.

lazy_static! {
    static ref BEHAVIOURS: [Behaviour; 3] = [
        Behaviour::new(harvest_wood, "harvest_wood"),
        Behaviour::new(mine_gold, "mine_gold"),
        Behaviour::new(get_reputation_or_gold, "get_reputation_or_gold"),
    ];
}

fn harvest_wood(current_actor_index: usize, tile: &mut Tile, rng: &mut StdRng) -> Result<(), String> {
    let resource_change: u32 = 1;
    if rng.gen_bool(1.0) { 
        let old_tile_resource_amount = *tile.resources.entry(Resource::Wood).or_insert(0);
        if possble_to_subtract(old_tile_resource_amount, resource_change) {
            *tile.resources.entry(Resource::Wood).or_insert(0) -= resource_change;
            *tile.actors[current_actor_index].resources.entry(Resource::Wood).or_insert(0) += resource_change;
            return Ok(())
        } else {
            return Err(String::from("Not enough wood to harvest."))
        }
    } else {
        Err(String::from("No luck."))
    }
}

fn mine_gold(current_actor_index: usize, tile: &mut Tile, rng: &mut StdRng) -> Result<(), String> {
    let resource_change: u32 = 1;
    if rng.gen_bool(0.5) { 
        let old_tile_resource_amount = *tile.resources.entry(Resource::Gold).or_insert(0);
        if possble_to_subtract(old_tile_resource_amount, resource_change) {
            *tile.resources.entry(Resource::Gold).or_insert(0) -= resource_change;
            *tile.actors[current_actor_index].resources.entry(Resource::Gold).or_insert(0) += resource_change;
            return Ok(())
        } else {
            return Err(String::from("Not enough gold to mine."))
        }
    } else {
        Err(String::from("No luck in gold mining."))
    }
}

fn get_reputation_or_gold(current_actor_index: usize, tile: &mut Tile, rng: &mut StdRng) -> Result<(), String> {
    let resource_change: u32 = 1;
    let mut other_actors = tile.actors.clone();
    other_actors.remove(current_actor_index);

    let max_reputation_among_other_actors = other_actors.iter()
        .map(|actor| actor.resources.get(&Resource::Reputation).unwrap_or(&0))
        .max()
        .unwrap_or(&0);

    let actor_reputation = tile.actors[current_actor_index]
        .resources
        .get(&Resource::Reputation)
        .unwrap_or(&0);

    if actor_reputation > max_reputation_among_other_actors {
        *tile.actors[current_actor_index].resources.entry(Resource::Gold).or_insert(0) += resource_change;
        Ok(())
    } else {
        *tile.actors[current_actor_index].resources.entry(Resource::Reputation).or_insert(0) += resource_change;
        Ok(())
    }
}

type Resources = BTreeMap<Resource, u32>;

#[derive(Debug, Clone)]
struct Behaviour {
    function: BehaviourFn,
    name: &'static str, // TODO Remove this field and struct
}

#[derive(Debug, Clone)]
struct BehaviourProb {
    behaviour: Behaviour,
    probability: f64,
}

#[derive(Debug, Clone)]
struct Actor {
    behaviours: Vec<BehaviourProb>,
    resources: Resources,
} 

#[derive(Debug, Default, Clone)]
struct Tile {
    actors: Vec<Actor>, 
    resources: Resources,
}

impl Behaviour {
    fn new(
        function: BehaviourFn,
        name:&'static str
    ) -> Self { Behaviour {function, name} }
}

impl BehaviourProb {
    fn new(behaviour: Behaviour, probability: f64) -> Self {
        BehaviourProb {behaviour, probability}
    }
}

impl Actor {
    fn new(resources: Resources, behaviours: Vec<BehaviourProb>) -> Actor {
        Actor {resources, behaviours}
    }

    fn get_utility(&self) -> f64 {
        let mut total_utility = 0.0;
        for (resource, &amount) in &self.resources {
            if amount > 0 {
                total_utility += f64::ln(amount as f64) + 1.0;
                    // We add constant to the {log of resource amount} because without it resource change from 0 to 1 will not change utility.
                    // This is so because ln(1.0) == 0.0.
            }
        }
        total_utility
    }

}

impl Tile {
    fn new(actors: Vec<Actor>, resources: Resources) -> Tile {
        Tile{actors, resources}
    }

    fn execute_behaviour(&mut self, rng: &mut StdRng, log: &mut String) {
        let actor_indices: Vec<usize> = (0..self.actors.len()).collect();
        for i in actor_indices {
            let chosen_behaviour: BehaviourFn = {
                let actor = &self.actors[i];
                let probabilities: Vec<f64> = actor.behaviours.iter().map(|b| b.probability).collect();
                let weighted_distribution = WeightedIndex::new(&probabilities).unwrap();
                let chosen_index = weighted_distribution.sample(rng);
                actor.behaviours[chosen_index].behaviour.function
            };
            let old_tile = self.clone();

            // First-come, first-served resource extraction system:
            // If the resource change is possible (thus behaviour is also possible) for the actor we are currently iterating over, the change will occur.
            // Consequently, other actors may fail in attempting to execute exactly the same behavior in the same game tick due to a lack of resources in the Tile.
            chosen_behaviour(i, self, rng);

            log_resource_changes(&old_tile, self, i, log)
        }
    }
}

fn log_resource_changes(initial_tile: &Tile, new_tile: &Tile, actor_index: usize, log: &mut String,) {
    for (resource, &initial_amount) in initial_tile.resources.iter() {
        let new_amount = new_tile.resources.get(resource).unwrap_or(&0);
        if initial_amount != *new_amount {
            log.push_str(&format!(
                "Actor {} made tile resource change: {:?} {} -> {}\n",
                actor_index, resource, initial_amount, new_amount
            ));
        }
    }

    let initial_actor = &initial_tile.actors[actor_index];
    let new_actor = &new_tile.actors[actor_index];
    for (resource, &initial_amount) in initial_actor.resources.iter() {
        let new_amount = new_actor.resources.get(resource).unwrap_or(&0);
        if initial_amount != *new_amount {
            log.push_str(&format!(
                "Actor {} resource change: {:?} {} -> {}\n",
                actor_index, resource, initial_amount, new_amount
            ));
        }
    }
    
}

fn possble_to_subtract(value: u32, amount_to_sustract: u32) -> bool {
    if amount_to_sustract > value {
        false
    } else {
        true
    }
}

fn generate_probability_distributions(actors_in_crossection: usize) -> Vec<Vec<f64>> {
    match actors_in_crossection {
        0 => {panic!("There should be at least one actor.")},
        1 => {
            let len = BEHAVIOURS.len();
            let probabilities_for_actor = vec![vec![1.0/(len as f64); len]];
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

    fn log_behaviour_probs<'a>(behaviour_probs: &[Vec<BehaviourProb>], log: &mut String) {
        log.push_str("Actor ID, Behaviours with probabilities: \n");
    
        for (actor_id, actor_behaviours) in behaviour_probs.iter().enumerate() {
            let row = actor_behaviours.iter().map(|bp| {
                format!("{} ({:.0}%)", bp.behaviour.name, bp.probability * 100.0)
            }).collect::<Vec<String>>().join(", ");
    
            log.push_str(&format!("{}, [{}]\n", actor_id, row));
        }
        log.push_str("\n");
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

        if let [HyperParam::NumOfProbValues(num_of_probability_values),
                HyperParam::GameTicks(num_of_game_ticks),
                HyperParam::GameSeed(game_seed),
                HyperParam::InitialTileResources(initial_tile_resources)
                ] = &hyper_params[..] {

                let mut log = String::new();
                log.push_str(&format!("Number of possible probability values for one behaviour: {},\nTotal game ticks: {},\nGame seed: {:?},\nInitial tile Resources: {:?}\n\n",
                num_of_probability_values, num_of_game_ticks, game_seed, &initial_tile_resources));

                let behaviour_probs: Vec<Vec<BehaviourProb>> = generate_probability_distributions(*num_of_probability_values)
                    .iter()
                    .map(|probs| 
                        BEHAVIOURS.iter()
                        .zip(probs.iter())
                        .map(|(behaviour, &probability)| BehaviourProb::new(behaviour.clone(), probability))
                        .collect()
                    )
                    .collect();

                log_behaviour_probs(&behaviour_probs, &mut log);

                let mut tile = Tile::new(vec![], initial_tile_resources.clone());
                for actor_behaviour_probs in behaviour_probs.iter() {
                    let actor = Actor::new(
                        BTreeMap::from([(Resource::Gold, 0), (Resource::Reputation, 0), (Resource::Wood, 0)]),
                        actor_behaviour_probs.clone()); // TODO Make bugfree initialisation with empty dict.
                    tile.actors.push(actor);
                }
            
                let mut rng = StdRng::from_seed(*game_seed);
                for t in 0..*num_of_game_ticks {
                    log.push_str(&format! ("\n---------- Game tick {} ----------\n", t));
                    tile.execute_behaviour(&mut rng, &mut log);
                }
                
            let (winner_index, winner) = tile.actors.iter().enumerate()
            .max_by(|(_, a), (_, b)| a.get_utility().partial_cmp(&b.get_utility()).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap(); 

            log.push_str(&format!("\nActor with this ID won: {:?}\nActor's resources are: {:?}\nActor's utility is: {:?}",
            winner_index,
            winner.resources,
            winner.get_utility()));

            let hash = hash_hyper_params(&hyper_params);
            let file_name = format!("output/{}.txt", hash);
            write(&file_name, log).unwrap();

        } else { panic!("Hyperparameters were not parsed correctly.") }
    });

    println!("Execution time: {:?}", timer.elapsed());
}
