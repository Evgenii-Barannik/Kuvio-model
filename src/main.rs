use std::fs;
use std::time::Instant;
use std::vec;
use std::collections::{BTreeMap, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};
use std::fs::write;
use itertools::{iproduct, Itertools};
use std::iter::zip;
use rand::distributions::{Distribution, WeightedIndex};
use rand::{Rng, rngs::StdRng, SeedableRng};
use lazy_static::lazy_static;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use plotters::{coord::Shift, prelude::*};
use std::collections::HashMap;
use toml::map::Map;
use toml::{Value, Table};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use rayon::prelude::*;
// use rayon::ThreadPoolBuilder;

// This enum exists to support iteration over possibly different datatypes inside variants.
// This is one of three complementary types.
#[derive(Debug, Clone, Hash)]
enum HyperParam { 
    ProbResolution(u64),
    GameTicks(u64),
    GameSeed(u64),
    // ResourceCollection(Resources),
}

// This tuple exists to make destructuring of current hyperparams more convenient.
// This is one of three complementary types.
type HyperParamCombination = (u64, u64, u64); 

/// This is one of three complementary types.
#[derive(Debug, Clone, Hash)]
struct HyperParamRanges { 
    probability_resolutions: Vec<HyperParam>,
    game_ticks: Vec<HyperParam>,
    game_seeds: Vec<HyperParam>,
    // resource_collections: Vec<HyperParam>,
}

// lazy_static! {
//     static ref INITIAL_RESOURCE_COMBINATIONS: Vec<HyperParam> = {
//         let gold_range = (1u32..=1u32).map(|x| 500 * x); 
//         let wood_range = (1u32..=1u32).map(|x| 1000 * x); 
        
//         iproduct!(gold_range, wood_range)
//         .map(|(gold_amount, wood_amount)| {
//             HyperParam::ResourceCollection(BTreeMap::from([(Resource::Gold, gold_amount), (Resource::Wood, wood_amount)]))
//         })
//         .collect::<Vec<HyperParam>>()
//     };
// }
        
#[derive(PartialEq, PartialOrd, Eq, Ord, Debug, Hash, Clone, EnumIter)]
enum Resource {
    Gold,
    Wood,
    Reputation,
}

type BehaviourFn = fn(usize, &mut Tile, &mut StdRng) -> Result<String, ()>; 

lazy_static! {
    static ref BEHAVIOURS: [BehaviourFn; 3] = 
    [harvest_wood, mine_gold, get_reputation_or_gold];
}

const BEHAVIOUR_NAMES: [&str; 3] = ["harvest_wood", "mine_gold", "get_reputation_or_gold"];

fn harvest_wood(current_actor_index: usize, tile: &mut Tile, _rng: &mut StdRng) -> Result<String, ()> {
    let resource_change: u32 = 1;
    let old_tile_resource_amount = *tile.resources.get(&Resource::Wood).unwrap_or(&0);
    let old_actor_resource_amount = *tile.actors[current_actor_index].resources.get(&Resource::Wood).unwrap_or(&0);

    if possble_to_subtract(old_tile_resource_amount, resource_change) {
        *tile.resources.entry(Resource::Wood).or_insert(0) -= resource_change;
        *tile.actors[current_actor_index].resources.entry(Resource::Wood).or_insert(0) += resource_change;

        Ok(String::from(format!("Wood {} -> {} for Tile | Wood {} -> {} for Actor {}.\n",
        old_tile_resource_amount,
        tile.resources.get(&Resource::Wood).unwrap_or(&0),
        old_actor_resource_amount,
        tile.actors[current_actor_index].resources.get(&Resource::Wood).unwrap_or(&0),
        current_actor_index)))
    } else {
        Ok(String::from("Not enough wood to harvest.\n"))
    }
}

fn mine_gold(current_actor_index: usize, tile: &mut Tile, rng: &mut StdRng) -> Result<String, ()> {
    if rng.gen_bool(0.5) { 
        let resource_change: u32 = 1;
        let old_actor_resource_amount = *tile.actors[current_actor_index].resources.get(&Resource::Gold).unwrap_or(&0);
        let old_tile_resource_amount = *tile.resources.entry(Resource::Gold).or_insert(0);

        if possble_to_subtract(old_tile_resource_amount, resource_change) {
            *tile.resources.entry(Resource::Gold).or_insert(0) -= resource_change;
            *tile.actors[current_actor_index].resources.entry(Resource::Gold).or_insert(0) += resource_change;

            Ok(String::from(format!("Gold {} -> {} for Tile | Gold {} -> {} for Actor {}.\n",
            old_tile_resource_amount,
            tile.resources.get(&Resource::Gold).unwrap_or(&0),
            old_actor_resource_amount,
            tile.actors[current_actor_index].resources.get(&Resource::Gold).unwrap_or(&0),
            current_actor_index)))
        } else {
            Ok(String::from("Not enough gold to mine.\n"))
        }
    } else {
        Ok(String::from("No luck in gold mining.\n"))
    }
}

fn get_reputation_or_gold(current_actor_index: usize, tile: &mut Tile, _rng: &mut StdRng) -> Result<String, ()> {
    let resource_change: u32 = 1;
    let mut other_actors = tile.actors.clone();
    other_actors.remove(current_actor_index);

    let max_reputation_among_other_actors = other_actors.iter()
        .map(|actor| actor.resources.get(&Resource::Reputation).unwrap_or(&0))
        .max()
        .unwrap_or(&0);

    let actor_reputation = tile.actors[current_actor_index].resources.get(&Resource::Reputation).unwrap_or(&0);

    if actor_reputation > max_reputation_among_other_actors {
        *tile.actors[current_actor_index].resources.entry(Resource::Gold).or_insert(0) += resource_change;
        Ok(String::from("Getting gold for the highest reputation.\n"))
    } else {
        *tile.actors[current_actor_index].resources.entry(Resource::Reputation).or_insert(0) += resource_change;
        Ok(String::from("Not enough reputation to get gold.\n"))
    }
}

type Resources = BTreeMap<Resource, u32>;

#[derive(Debug, Clone)]
struct BehaviourProb {
    behaviour: BehaviourFn,
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

impl Actor {
    fn new(initial_resources: Resources, behaviour_probs: Vec<f64>) -> Actor {
        let mut zeroed_resources = Resource::iter().map(|r| (r, 0)).collect::<Resources>();
        
        for (resource, amount) in initial_resources {
            zeroed_resources.insert(resource, amount);
        }
        
        let behaviour_probs = zip(BEHAVIOURS.into_iter(), behaviour_probs.into_iter())
            .map(|(behaviour, probability)| BehaviourProb {behaviour, probability})
            .collect();

        Actor {resources: zeroed_resources, behaviours: behaviour_probs}
    }

    fn get_utility(&self) -> f64 {
        let mut total_utility = 0.0;
        for (_resource, &amount) in &self.resources {
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
                actor.behaviours[chosen_index].behaviour
            };

            // First-come, first-served resource extraction system:
            // If the resource change is possible (thus behaviour is also possible) for the actor we are currently iterating over, the change will occur.
            // Consequently, other actors may fail in attempting to execute exactly the same behavior in the same game tick due to a lack of resources in the Tile.
            let result = chosen_behaviour(i, self, rng);
            
            log.push_str(&result.ok().unwrap())
        }
    }
}

fn possble_to_subtract(value: u32, amount_to_substract: u32) -> bool {
    if amount_to_substract > value {
        false
    } else {
        true
    }
}

fn generate_probability_distributions(actors_in_crossection: u64) -> Vec<Vec<f64>> {
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
                (BEHAVIOURS.len() - 1).try_into().unwrap(),
                actors_in_crossection,
            );
            
            probabilities_for_all_actors
        }
    }
}

fn probability_distributions_recursion(
    probabilities_for_all_actors: &mut Vec<Vec<f64>>,
    probabilities_for_actor: &mut Vec<f64>,
    remaining_probability_steps: u64,
    remaining_recursion_depth: u64,
    actors_in_crossection: u64,
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

fn log_behaviour_probs(behaviour_probs: &Vec<Vec<f64>>, log: &mut String) {
    log.push_str("Actor ID, Behaviours with probabilities: \n");

    for (actor_number, actor_behaviours) in behaviour_probs.iter().enumerate() {
        let row = actor_behaviours.iter().enumerate().map(|(i, behaviour_probability)| {
            let behaviour_name = BEHAVIOUR_NAMES[i];
            format!("{} ({:.0}%)", behaviour_name, behaviour_probability * 100.0)
        }).collect::<Vec<String>>().join(", ");

        log.push_str(&format!("{}, [{}]\n", actor_number, row));
    }
    log.push_str("\n");
}

fn hash_hyper_params(hyper_params: &HyperParamCombination) -> u64 {
    let mut hasher = DefaultHasher::new();
    hyper_params.hash(&mut hasher);
    hasher.finish()
}

macro_rules! for_each_hyperparam_combination {
    ($callback:expr) => {{
        let hps = read_settings_toml().unwrap();

        vec![&hps.probability_resolutions,
             &hps.game_ticks,
             &hps.game_seeds,
            //  &HYPERPARAM_RANGES.resource_collections
             ]
            .into_iter()
            .multi_cartesian_product()
            .collect::<Vec<_>>()
            .into_par_iter()
            .for_each(|hyperparams| {
                if let [HyperParam::ProbResolution(probability_resolutions),
                        HyperParam::GameTicks(game_ticks),
                        HyperParam::GameSeed(game_seeds),
                        // HyperParam::ResourceCollection(resource_collections)
                       ] = &hyperparams[..] {
                    
                    $callback((*probability_resolutions, *game_ticks, *game_seeds));

                } else {
                    panic!("Hyperparameters were not parsed correctly.");
                }
            });
        }};
    }
    
fn plot_utility_distribution(
    actors: &Vec<Actor>,
    behavior_probs: &Vec<Vec<f64>>,
    root: &mut DrawingArea<BitMapBackend<'_>, Shift>,
    tick_number: usize,
) {
        
    let utilities: Vec<f64> = actors.iter()
    .map(|actor| actor.get_utility())
    .collect();

    let max_utility: f64 = *utilities.iter()
    .max_by(|a, b| a.partial_cmp(b).unwrap())
    .unwrap();

    let plot_height = 5u32;
    root.fill(&WHITE).unwrap();
    let mut chart = ChartBuilder::on(&root)
        .margin(5)
        .caption("Utility Distribution", ("sans-serif", 30))
        .x_label_area_size(40)
        .y_label_area_size(40)
        .build_cartesian_2d(0.0..max_utility, 0..plot_height)
        .unwrap();
    chart.configure_mesh().x_desc("Utility").y_desc("N").draw().unwrap();

    let bucket_count = 100;
    let bucket_width = max_utility / bucket_count as f64;
    let mut buckets = vec![0u32; bucket_count];
    for (actor_index, utility) in utilities.iter().enumerate() {
        let bucket_index = ((utility / max_utility) * (bucket_count as f64 - 1.0)).floor() as usize;
        let color = RGBColor(
            (255.0 * behavior_probs[actor_index][0]) as u8,
            (255.0 * behavior_probs[actor_index][1]) as u8,
            (255.0 * behavior_probs[actor_index][2]) as u8,
        );

        let bar_left = bucket_index as f64 * bucket_width;
        let bar_right = bar_left + bucket_width;
        let bar_bottom = buckets[bucket_index];
        let bar_top = bar_bottom + 1;

        chart.draw_series(std::iter::once(Rectangle::new(
            [(bar_left, bar_bottom), (bar_right, bar_top)],
            color.filled(),
        ))).unwrap();

        buckets[bucket_index]+= 1;
    }

    let (legend_x, legend_y) = (50, 50);
    let legend_size = 15;
    let text_gap = 5;
    let text_size = 15;

    let tick_info = format!("Tick: {}", tick_number);

    let legend_entries = vec![
        ("harvest_wood", RED),
        ("mine_gold", GREEN),
        ("get_reputation_or_gold", BLUE),
        (&tick_info, WHITE)
    ];

    for (i, (label, color)) in legend_entries.iter().enumerate() {
        let y_position = legend_y + i as i32 * (legend_size + text_gap + text_size);

        root.draw(&Rectangle::new(
            [(legend_x, y_position), (legend_x + legend_size, y_position + legend_size)],
            color.filled(),
        )).unwrap();

        root.draw(&Text::new(
            *label,
            (legend_x + legend_size + text_gap, y_position + (legend_size / 2)),
            ("sans-serif", text_size).into_font(),
        )).unwrap();
    }

    root.present().unwrap();
}

pub fn try_to_read_field_as_vec(map: &Map<String, Value>, key: &str) -> Option<Vec<u64>> {
    map.get(key).and_then(|value| match value {
        Value::Array(arr) => Some(arr.iter().filter_map(Value::as_integer).map(|num| num as u64).collect()),
        _ => None,
    })
}

fn read_settings_toml() -> Option<HyperParamRanges> {
    let toml_files: Vec<PathBuf> = WalkDir::new("settings")
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| 
            entry.file_type().is_file() &&
            entry.file_name().to_string_lossy().ends_with(".toml"))
        .map(|entry| entry.into_path() )
        .collect();
    
    for file in &toml_files {
        if file.file_name().unwrap() == "settings.toml" {
            println!("{:?} found", file);
            let contents = fs::read_to_string(file).unwrap();
            let toml_map: Value = contents.parse().unwrap();
    
            if let Some(Value::Array(hp_map)) = toml_map.get("Hyperparameters") {
                for hp in hp_map {
                    let game_seeds = read_hyperparameter_vec(hp, "game_seeds").unwrap();
                    let game_ticks = read_hyperparameter_vec(hp, "game_ticks").unwrap();
                    let probability_resolutions = read_hyperparameter_vec(hp, "probability_resolutions").unwrap();
                    let initial_tile_gold = read_hyperparameter_vec(hp, "initial_tile_gold");
                    let initial_tile_wood = read_hyperparameter_vec(hp, "initial_tile_wood");

                    let hp_ranges = HyperParamRanges {
                        game_seeds: game_seeds.into_iter().map(HyperParam::GameSeed).collect_vec(),
                        game_ticks: game_ticks.into_iter().map(HyperParam::GameTicks).collect_vec(),
                        probability_resolutions: probability_resolutions.into_iter().map(HyperParam::ProbResolution).collect(),
                    };

                    return Some(hp_ranges)
                }
            }
        
        }
    } 
    return None
}
    
fn read_hyperparameter_vec(hyperparameter: &Value, key: &str) -> Option<Vec<u64>> {
    if let Some(Value::Array(values)) = hyperparameter.get(key) {
        let extracted_values: Vec<u64> = values.iter().filter_map(|v| {
            if let Value::Integer(value) = v { Some(*value as u64) } else { None }
        }).collect();
        println!("Read {}: {:?}", key, extracted_values);
        return Some(extracted_values)
    }
    else {None}
}


fn main() {
    let timer = Instant::now();
    fs::create_dir_all("output").unwrap();

    // rayon::ThreadPoolBuilder::new().num_threads(1).build_global().unwrap();
        for_each_hyperparam_combination!(|hyperparams: HyperParamCombination| {
            let (num_of_prob_values, num_of_game_ticks, game_seed) = hyperparams;

            let initial_tile_resources = BTreeMap::from([(Resource::Gold, 500), (Resource::Wood, 1000)]);
            let mut log = String::new();
            log.push_str(&format!("Number of possible probability values for one behaviour: {},\nTotal game ticks: {},\nGame seed: {:?},\nInitial tile Resources: {:?}\n\n",
            num_of_prob_values, num_of_game_ticks, game_seed, &initial_tile_resources));
            
            let behaviour_probs = generate_probability_distributions(num_of_prob_values);
            log_behaviour_probs(&behaviour_probs, &mut log);
            
            let mut tile = Tile::new(vec![], initial_tile_resources.clone());
            for actors_behaviour_probs in behaviour_probs.iter() {
                let actor = Actor::new(BTreeMap::new(), actors_behaviour_probs.clone());
                // Actors should be in the same order as behaviour_probs due to the way actors were created.
                tile.actors.push(actor);
            }
            
            let hash = hash_hyper_params(&hyperparams);
            let plot_file_name = format!("output/{}.gif", hash);
            let mut root = BitMapBackend::gif(plot_file_name, (640, 480), 100).unwrap().into_drawing_area();
            let mut rng = StdRng::seed_from_u64(game_seed as u64);
            for t in 0..num_of_game_ticks {
                log.push_str(&format! ("\n---------- Game tick {} ----------\n", t));
                tile.execute_behaviour(&mut rng, &mut log);
                plot_utility_distribution(&tile.actors, &behaviour_probs, &mut root, (t as u64).try_into().unwrap());
            }

        let (winner_index, winner) = tile.actors.iter().enumerate()
        .max_by(|(_, a), (_, b)| a.get_utility().partial_cmp(&b.get_utility()).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap(); 

        log.push_str(&format!("\nActor with this ID won: {:?}\nActor's resources are: {:?}\nActor's utility is: {:?}",
        winner_index, winner.resources, winner.get_utility()));

        let file_name = format!("output/{}.txt", hash);
        write(&file_name, log).unwrap();
        });

    println!("Execution time: {:?}", timer.elapsed());
}
    