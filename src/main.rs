use std::fs;
use std::time::Instant;
use std::vec;
use std::collections::{BTreeMap, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};
use std::fs::write;
use itertools::{iproduct, Itertools};
use rand::distributions::{Distribution, WeightedIndex};
use rand::{Rng, rngs::StdRng, SeedableRng};
use lazy_static::lazy_static;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use plotters::prelude::*;
use rayon::prelude::*;
// use rayon::ThreadPoolBuilder;

// This enum exists to support iteration over possibly different datatypes inside variants. 
#[derive(Debug, Clone, Hash)]
enum HyperParam {
    ProbResolution(usize),
    GameTicks(usize),
    GameSeed(u64),
    ResourceCollection(Resources),
}

type HyperParamCombination = (usize, usize, u64, Resources);

#[derive(Debug, Clone, Hash)]
struct HyperParamRanges {
    probability_resolutions: Vec<HyperParam>,
    game_ticks: Vec<HyperParam>,
    game_seeds: Vec<HyperParam>,
    resource_collections: Vec<HyperParam>,
}

// Hyperparameter ranges that define region of phase space that we explore.
lazy_static! {
    static ref HYPERPARAM_RANGES: HyperParamRanges = HyperParamRanges {
        probability_resolutions: (6..=6).map(HyperParam::ProbResolution).collect_vec(),
        game_ticks: [500, 5000].map(HyperParam::GameTicks).to_vec(),
        game_seeds: [2].map(HyperParam::GameSeed).to_vec(),
        resource_collections: INITIAL_RESOURCE_COMBINATIONS.to_vec(),
    };
}

lazy_static! {
    static ref INITIAL_RESOURCE_COMBINATIONS: Vec<HyperParam> = {
        let gold_range = (1u32..=4u32).map(|x| 500 * x); 
        let wood_range = (1u32..=4u32).map(|x| 1000 * x); 
        
        iproduct!(gold_range, wood_range)
        .map(|(gold_amount, wood_amount)| {
            HyperParam::ResourceCollection(BTreeMap::from([(Resource::Gold, gold_amount), (Resource::Wood, wood_amount)]))
        })
        .collect::<Vec<HyperParam>>()
    };
}
        
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

impl BehaviourProb {
    fn new(behaviour: BehaviourFn, probability: f64) -> Self {
        BehaviourProb {behaviour, probability}
    }
}

impl Actor {
    fn new(initial_resources: Resources, behaviours: Vec<BehaviourProb>) -> Actor {
        let mut zeroed_resources = Resource::iter().map(|r| (r, 0)).collect::<Resources>();
        
        for (resource, amount) in initial_resources {
            zeroed_resources.insert(resource, amount);
        }
        
        Actor {resources: zeroed_resources, behaviours}
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
            let row = actor_behaviours.iter().enumerate().map(|(i, bp)| {
                let behaviour_name = BEHAVIOUR_NAMES[i];
                format!("{} ({:.0}%)", behaviour_name, bp.probability * 100.0)
            }).collect::<Vec<String>>().join(", ");
    
            log.push_str(&format!("{}, [{}]\n", actor_id, row));
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
        vec![&HYPERPARAM_RANGES.probability_resolutions,
             &HYPERPARAM_RANGES.game_ticks,
             &HYPERPARAM_RANGES.game_seeds,
             &HYPERPARAM_RANGES.resource_collections]
            .into_iter()
            .multi_cartesian_product()
            .collect::<Vec<_>>()
            .into_par_iter()
            .for_each(|hyperparams| {
                if let [HyperParam::ProbResolution(probability_resolutions),
                        HyperParam::GameTicks(game_ticks),
                        HyperParam::GameSeed(game_seeds),
                        HyperParam::ResourceCollection(resource_collections)
                       ] = &hyperparams[..] {
                    
                    $callback((*probability_resolutions, *game_ticks, *game_seeds, resource_collections.clone()));

                } else {
                    panic!("Hyperparameters were not parsed correctly.");
                }
            });
    }};
}

fn plot_utility_distribution(utilities: &[f64], behavior_probabilities: &Vec<Vec<f64>>, file_path: &str) {
    let max_utility = *utilities.iter().max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap();
    let max_height = 5u32;
    let bucket_count = 100;
    
    let mut buckets = vec![vec![0u32; behavior_probabilities.len()]; bucket_count];
    for (i, &utility) in utilities.iter().enumerate() {
        let index = ((utility / max_utility) * (bucket_count as f64 - 1.0)).floor() as usize;
        let behavior_index = i % behavior_probabilities.len();
        buckets[index][behavior_index] += 1;
    }

    let plot = BitMapBackend::new(file_path, (640, 480)).into_drawing_area();
    plot.fill(&WHITE).unwrap();

    let mut chart = ChartBuilder::on(&plot)
        .margin(5)
        .caption("Utility Distribution", ("sans-serif", 30))
        .x_label_area_size(40)
        .y_label_area_size(40)
        .build_cartesian_2d(0.0..max_utility, 0..max_height)
        .unwrap();

    chart.configure_mesh().x_desc("Utility").y_desc("N").draw().unwrap();

    let bucket_width = max_utility / bucket_count as f64;

    for (index, bucket) in buckets.iter().enumerate() {
        let mut accumulated_height = 0;
        for (behavior_index, &count) in bucket.iter().enumerate() {
            let color = RGBColor(
                (255.0 * behavior_probabilities[behavior_index][0]) as u8,
                (255.0 * behavior_probabilities[behavior_index][1]) as u8,
                (255.0 * behavior_probabilities[behavior_index][2]) as u8,
            );

            let bar_left = index as f64 * bucket_width;
            let bar_right = bar_left + bucket_width;
            let bar_bottom = accumulated_height;
            let bar_top = accumulated_height + count;

            chart.draw_series(std::iter::once(Rectangle::new(
                [(bar_left, bar_bottom as u32), (bar_right, bar_top)],
                color.filled(),
            ))).unwrap();

            accumulated_height = bar_top;
        }
    }

    let (legend_x, legend_y) = (50, 50);
    let legend_size = 15;
    let text_gap = 5;
    let text_size = 15;

    let legend_entries = vec![
        ("harvest_wood", RED),
        ("mine_gold", GREEN),
        ("get_reputation_or_gold", BLUE),
    ];

    for (i, (label, color)) in legend_entries.iter().enumerate() {
        let y_position = legend_y + i as i32 * (legend_size + text_gap + text_size);

        plot.draw(&Rectangle::new(
            [(legend_x, y_position), (legend_x + legend_size, y_position + legend_size)],
            color.filled(),
        )).unwrap();

        plot.draw(&Text::new(
            *label,
            (legend_x + legend_size + text_gap, y_position + (legend_size / 2)),
            ("sans-serif", text_size).into_font(),
        )).unwrap();
    }
}


fn main() {
    let timer = Instant::now();
    fs::create_dir_all("output").unwrap();

    // rayon::ThreadPoolBuilder::new().num_threads(1).build_global().unwrap();
        for_each_hyperparam_combination!(|hyperparams: HyperParamCombination| {
            let (num_of_prob_values, num_of_game_ticks, game_seed, ref initial_tile_resources) = hyperparams;

            let mut log = String::new();
            log.push_str(&format!("Number of possible probability values for one behaviour: {},\nTotal game ticks: {},\nGame seed: {:?},\nInitial tile Resources: {:?}\n\n",
            num_of_prob_values, num_of_game_ticks, game_seed, &initial_tile_resources));

            let behaviour_probs: Vec<Vec<BehaviourProb>> = generate_probability_distributions(num_of_prob_values)
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
                let actor = Actor::new(BTreeMap::new(), actor_behaviour_probs.clone());
                tile.actors.push(actor);
            }
        
            let mut rng = StdRng::seed_from_u64(game_seed as u64);
            for t in 0..num_of_game_ticks {
                log.push_str(&format! ("\n---------- Game tick {} ----------\n", t));
                tile.execute_behaviour(&mut rng, &mut log);
            }

        let hash = hash_hyper_params(&hyperparams);

        let mut utilities: Vec<f64> = vec![];
        for actor in &tile.actors {
            utilities.push(actor.get_utility())
        }
        let plot_file_name = format!("output/{}_hist.png", hash);

        let behavior_probabilities: Vec<Vec<f64>> = tile.actors.iter()
        .map(|actor| actor.behaviours.iter().map(|b| b.probability).collect())
        .collect();

        plot_utility_distribution(&utilities, &behavior_probabilities, &plot_file_name);

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
