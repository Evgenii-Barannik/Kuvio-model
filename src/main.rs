use std::fs;
use std::time::Instant;
use std::vec;
use std::collections::{BTreeMap, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};
use std::fs::write;
use itertools::Itertools;
use std::iter::zip;
use rand::distributions::{Distribution, WeightedIndex};
use rand::{Rng, rngs::StdRng, SeedableRng};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use plotters::{coord::Shift, prelude::*};
use toml::map::Map;
use toml::Value;
use std::path::PathBuf;
use walkdir::WalkDir;
use rayon::prelude::*;
// use rayon::ThreadPoolBuilder;

// This enum exists to support iteration over possibly different types inside variants.
// This is one of three complementary types.
#[derive(Debug, Clone, Hash)]
enum HyperParam { 
    // This variant can be made to contain usize num (that can be cast to u64 before use because u64 is required),
    // but we want to retain type variability for hyperparameters anyway.
    GameSeed(u64), 
    GameTickCount(usize),
    ProbabilityResolution(usize),
    InitialTileGold(usize),
}

// This tuple exists to make destructuring of hyperparameter combinations more convenient.
// This is one of three complementary types.
type HyperParamCombination = (u64, usize, usize, usize); 

/// This is one of three complementary types.
#[derive(Debug, Clone, Hash)]
struct HyperParamRanges { 
    game_seed_values: Vec<HyperParam>,
    game_tick_count_values: Vec<HyperParam>,
    probability_resolution_values: Vec<HyperParam>,
    initial_tile_gold_values: Vec<HyperParam>,
}
        
#[derive(PartialEq, PartialOrd, Eq, Ord, Debug, Hash, Clone, EnumIter)]
enum Resource {
    Gold,
}

type AgentIndex = usize;
type BehaviourFn = fn(AgentIndex, &mut Tile, &mut StdRng) -> Result<String, ()>; 
const BEHAVIOURS: [BehaviourFn; 3] = [spend_gold_to_increase_reputation, mine_gold, try_to_collect_tax];
const BEHAVIOUR_NAMES: [&str; 3] = ["spend_gold_to_increase_reputation", "mine_gold", "try_to_collect_tax"];

fn mine_gold(current_actor_index: AgentIndex, tile: &mut Tile, rng: &mut StdRng) -> Result<String, ()> {
    let resource_change: usize = 1;
    let old_actor_resource_amount = *tile.actors[current_actor_index].resources.get(&Resource::Gold).unwrap_or(&0);
    let old_tile_resource_amount = *tile.resources.entry(Resource::Gold).or_insert(0);

    if possble_to_subtract(old_tile_resource_amount, resource_change)
    && rng.gen_bool(0.5) {
        *tile.resources.entry(Resource::Gold).or_insert(0) -= resource_change;
        *tile.actors[current_actor_index].resources.entry(Resource::Gold).or_insert(0) += resource_change;

        Ok(String::from(format!("Gold {} -> {} for Tile | Gold {} -> {} for Actor {}.\n",
        old_tile_resource_amount,
        tile.resources.get(&Resource::Gold).unwrap_or(&0),
        old_actor_resource_amount,
        tile.actors[current_actor_index].resources.get(&Resource::Gold).unwrap_or(&0),
        current_actor_index)))
    } else {
        Ok(String::from("Not enough gold to mine or no luck.\n"))
    }
}

fn try_to_collect_tax(calling_agent_index: AgentIndex, tile: &mut Tile, rng: &mut StdRng) -> Result<String, ()> {
    let resource_change: usize = 1;
    let mut total_tax_collected: usize = 0;

    for (targeted_actor_index, _targeted_actor) in tile.clone().actors.iter().enumerate() {
        let calling_actor_reputation_about_target = tile.reputations[calling_agent_index][targeted_actor_index];
        let targeted_actor_reputation_about_caller = tile.reputations[targeted_actor_index][calling_agent_index];
        let target_actor_gold_amount = *tile.actors[targeted_actor_index].resources.get(&Resource::Gold).unwrap_or(&0);

        if (targeted_actor_reputation_about_caller > calling_actor_reputation_about_target)
        && possble_to_subtract(target_actor_gold_amount, resource_change) 
        && rng.gen_bool(0.2) {
            *tile.actors[calling_agent_index].resources.entry(Resource::Gold).or_insert(0) += resource_change;
            *tile.actors[targeted_actor_index].resources.entry(Resource::Gold).or_insert(0) -= resource_change;
            total_tax_collected += 1;
        }
    }

    Ok(String::from(format!("Agent {} collected {} gold from taxes.\n", calling_agent_index, total_tax_collected)))
}

fn spend_gold_to_increase_reputation(calling_agent: AgentIndex, tile: &mut Tile, _rng: &mut StdRng) -> Result<String, ()> {
    let old_actor_gold_amount = *tile.actors[calling_agent].resources.get(&Resource::Gold).unwrap_or(&0);
    let gold_required = tile.actors.len() - 1;

    if possble_to_subtract(old_actor_gold_amount, gold_required) {
        *tile.actors[calling_agent].resources.entry(Resource::Gold).or_insert(0) -= gold_required;
        let indices_of_other_actors = {
            let mut agent_indices = (0..tile.actors.len()).collect_vec();
            agent_indices.remove(calling_agent);
            agent_indices
        };
        
        for index in indices_of_other_actors {
            *tile.actors[index].resources.entry(Resource::Gold).or_insert(0) += 1;
        }

        tile.reputations.update_reputations_of_others_about_agent(0, |x| *x += 1.0);
        Ok(String::from("Gold spent to buy reputation.\n"))

    } else {
        Ok(String::from("Not enough gold to pay.\n"))
    }

}

type Resources = BTreeMap<Resource, usize>;

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

type ReputationMatrix = Vec<Vec<f64>>;

fn log_reputations(m: &ReputationMatrix, log: &mut String) {
    log.push_str("Reputation matrix: \n");
    let header: String = (0..m[0].len())
        .map(|col_index| format!("{:5}", col_index))
        .collect::<Vec<String>>()
        .join(" ");
    log.push_str(&format!("IDs{}\n", header));

    for (row_index, row) in m.iter().enumerate() {
        let row_str = row.iter()
            .map(|&val| format!("{:5.2}", val))
            .collect::<Vec<String>>()
            .join(" ");
        log.push_str(&format!("{:2}  {}\n", row_index, row_str));
    }
}

fn log_behaviour_probs(behaviour_probs: &Vec<Vec<f64>>, log: &mut String) {
    log.push_str("IDs and behaviours with probabilities: \n");

    for (actor_number, actor_behaviours) in behaviour_probs.iter().enumerate() {
        let row = actor_behaviours.iter().enumerate().map(|(i, behaviour_probability)| {
            let behaviour_name = BEHAVIOUR_NAMES[i];
            format!("{} ({:.0}%)", behaviour_name, behaviour_probability * 100.0)
        }).collect::<Vec<String>>().join(", ");

        log.push_str(&format!("{:2}   [{}]\n", actor_number, row));
    }
    log.push_str("\n");
}
fn log_general_information (hyperparameters: &HyperParamCombination, log: &mut String) {
    let (game_seed, game_tick_count, probability_resolution, initial_tile_gold) = hyperparameters;
    log.push_str(&format!("Number of possible probability values for one behaviour: {},\nGame ticks count: {},\nGame seed: {:?},\nInitial tile gold: {:?}\n\n",
    probability_resolution, game_tick_count, game_seed, &initial_tile_gold));
}
fn log_resources (agents: &Vec<Actor>, log: &mut String) {
    log.push_str("IDs and final resources:\n");
    for (id, agent) in agents.iter().enumerate() {
        log.push_str(&format!("{:2}  {:?}\n", id, &agent.resources));
    }
    log.push_str("\n");
}

#[derive(Debug, Default, Clone)]
struct Tile {
    actors: Vec<Actor>, 
    resources: Resources,
    reputations: ReputationMatrix,
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

}

impl Tile {
    fn new(actors: Vec<Actor>, resources: Resources, reputations: Vec<Vec<f64>>) -> Tile {
        Tile{actors, resources, reputations}
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

trait ReputationsTrait {
    fn update_reputations_of_agent_about_others<F>(&mut self, agent_index: usize, update_fn: F)
    where F: Fn(&mut f64);

    fn update_reputations_of_others_about_agent<F>(&mut self, agent_index: usize, update_fn: F)
    where F: Fn(&mut f64);
}

impl ReputationsTrait for ReputationMatrix {
    fn update_reputations_of_agent_about_others<F>(&mut self, agent_index: usize, update_fn: F)
    where F: Fn(&mut f64) {
        if let Some(row) = self.get_mut(agent_index) {
            for reputation in row {
                update_fn(reputation);
            }
        }
    }

    fn update_reputations_of_others_about_agent<F>(&mut self, agent_index: usize, update_fn: F)
    where F: Fn(&mut f64) {
        for row in self.iter_mut() {
            if let Some(rep) = row.get_mut(agent_index) {
                update_fn(rep);
            }
        }
    }
}


fn possble_to_subtract(value: usize, amount_to_substract: usize) -> bool {
    if amount_to_substract <= value {
        true
    } else {
        false
    }
}


fn generate_probability_distributions(number_of_probability_values: usize) -> Vec<Vec<f64>> {
    match number_of_probability_values {
        0 => {panic!("There should be at least one probability value in range.")},
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
                number_of_probability_values - 1,
                BEHAVIOURS.len() - 1,
                number_of_probability_values,
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
    number_of_probability_values: usize,
) {
    let probability_step: f64 = 1.0 / (number_of_probability_values - 1) as f64;
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
                number_of_probability_values,
            );
        }
    }
}

fn hash_hyper_params(hyper_params: &HyperParamCombination) -> u64 {
    let mut hasher = DefaultHasher::new();
    hyper_params.hash(&mut hasher);
    hasher.finish()
}

macro_rules! for_each_hyperparam_combination {
    ($callback:expr) => {{
        let (hps, settings) = read_config();
        vec![
             &hps.game_seed_values,
             &hps.game_tick_count_values,
             &hps.probability_resolution_values,
             &hps.initial_tile_gold_values,
             ]
            .into_iter()
            .multi_cartesian_product()
            .collect::<Vec<_>>()
            .into_par_iter()
            .for_each(|hyperparams| {
                if let [
                        HyperParam::GameSeed(game_seed),
                        HyperParam::GameTickCount(game_tick_count),
                        HyperParam::ProbabilityResolution(probability_resolution),
                        HyperParam::InitialTileGold(initial_tile_gold),
                       ] = &hyperparams[..] {
                        
                        $callback(((
                            *game_seed,
                            *game_tick_count,
                            *probability_resolution,
                            *initial_tile_gold,
                        ), settings.clone()));
                } else {
                    panic!("Hyperparameters were not parsed correctly.");
                }
            });
        }};
    }
    
fn plot_gold_distribution(
    actors: &Vec<Actor>,
    behavior_probs: &Vec<Vec<f64>>,
    root: &mut DrawingArea<BitMapBackend<'_>, Shift>,
    tick_number: usize,
) {
        
    let log_resources: Vec<f64> = actors.iter()
    .map(|actor| f64::log10(*actor.resources.get(&Resource::Gold).unwrap() as f64))
    .collect();

    let max_log_resource: f64 = *log_resources.iter()
    .max_by(|a, b| a.partial_cmp(b).unwrap())
    .unwrap();

    let plot_height = 5u32;
    root.fill(&WHITE).unwrap();
    let mut chart = ChartBuilder::on(&root)
        .margin(5)
        .caption("Resource distribution", ("sans-serif", 30))
        .x_label_area_size(40)
        .y_label_area_size(40)
        .build_cartesian_2d(0.0..max_log_resource, 0..plot_height)
        .unwrap();
    chart.configure_mesh().x_desc("log10(Resource)").y_desc("N").draw().unwrap();

    let bucket_count = 100;
    let bucket_width = max_log_resource / bucket_count as f64;
    let mut buckets = vec![0u32; bucket_count];
    for (actor_index, log_resource) in log_resources.iter().enumerate() {
        let bucket_index = ((log_resource / max_log_resource) * (bucket_count as f64 - 1.0)).floor() as usize;
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

    let (legend_x, legend_y) = (55, 55);
    let legend_size = 15;
    let text_gap = 5;
    let text_size = 15;

    let tick_info = format!("Tick: {}", tick_number);

    let legend_entries = vec![
        (BEHAVIOUR_NAMES[0], RED),
        (BEHAVIOUR_NAMES[1], GREEN),
        (BEHAVIOUR_NAMES[2], BLUE),
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

pub fn try_to_read_field_as_vec(map: &Map<String, Value>, key: &str) -> Option<Vec<usize>> {
    map.get(key).and_then(|value| match value {
        Value::Array(arr) => Some(arr.iter().filter_map(Value::as_integer).map(|num| num as usize).collect()),
        _ => None,
    })
}

#[derive(Debug, Clone)]
struct Settings {
    plotting_frame_subselection_factor: usize,
    print_game_logs: bool,
}

fn read_config() -> (HyperParamRanges, Settings) {
    let toml_files: Vec<PathBuf> = WalkDir::new(".")
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| 
            entry.file_type().is_file() &&
            entry.file_name().to_string_lossy().ends_with(".toml"))
        .map(|entry| entry.into_path() )
        .collect();

    let mut settings = Settings { 
        plotting_frame_subselection_factor: 1usize, // Default values
        print_game_logs: true, // Default values
    };
    
    for file in &toml_files {
        if file.file_name().unwrap() == "config.toml" {
            println!("File {:?} found", file);
            let toml_map: Value = fs::read_to_string(file).unwrap().parse().unwrap();

            if let Some(Value::Array(settings_map)) = toml_map.get("Settings") {
                for setting in settings_map {

                    let setting1 = "plotting_frame_subselection_factor";
                    let setting2 = "print_game_logs";
                    if let Some(value) = setting.get(setting1) {
                        let extracted_value = value.as_integer().unwrap() as usize;
                        settings.plotting_frame_subselection_factor = extracted_value;
                        println!("{}: {:?}", setting1, extracted_value);
                    }
                    if let Some(value) = setting.get(setting2) {
                        let extracted_value = value.as_bool().unwrap();
                        settings.print_game_logs = extracted_value;
                        println!("{}: {:?}", setting2, extracted_value);
                    }
                };
            }
            
            if let Some(Value::Array(hp_map)) = toml_map.get("Hyperparameters") {
                for hp in hp_map {
                    let game_seed_values = read_vec_entry(hp, "game_seed_values")
                        .expect("File config.toml should contain game_seed_values entry in [[Hyperparameters]] with at least one usize value in list.")
                        .into_iter().map(|seed| HyperParam::GameSeed(seed as u64)).collect();
                    let game_tick_count_values = read_vec_entry(hp, "game_tick_count_values")
                        .expect("File config.toml should contain game_tick_count_values entry in [[Hyperparameters]] with at least one usize value in list.")
                        .into_iter().map(HyperParam::GameTickCount).collect();
                    let probability_resolution_values = read_vec_entry(hp, "probability_resolution_values")
                        .expect("File config.toml should contain probability_resolution_values entry in [[Hyperparameters]] with at least one usize value in list.")
                        .into_iter().map(HyperParam::ProbabilityResolution).collect();
                    let initial_tile_gold_values = read_vec_entry(hp, "initial_tile_gold_values")
                        .expect("File config.toml should contain initial_tile_gold_values entry in [[Hyperparameters]] with at least one usize value in list.")
                        .into_iter().map(HyperParam::InitialTileGold).collect();
                    let hp_ranges = HyperParamRanges {
                        game_seed_values,
                        game_tick_count_values,
                        probability_resolution_values,
                        initial_tile_gold_values,
                    };

                    return  (hp_ranges, settings)
                }
            } else {panic!("[[Hyperparameters]] section was not found in config.toml") }
        }
    } panic!("config.toml was not found") 
}

fn read_vec_entry(hyperparameter: &Value, key: &str) -> Option<Vec<usize>> {
    if let Some(Value::Array(values)) = hyperparameter.get(key) {
        let extracted_values: Vec<usize> = values.iter().filter_map(|v| {
            if let Value::Integer(value) = v { Some(*value as usize) } else { None }
        }).collect();
        println!("{}: {:?}", key, extracted_values);
        Some(extracted_values)
    } else {
        None
    }
}

fn main() {
    let timer = Instant::now();
    fs::create_dir_all("output").unwrap();

    // rayon::ThreadPoolBuilder::new().num_threads(1).build_global().unwrap();
    for_each_hyperparam_combination!(|(hyperparams, settings): (HyperParamCombination, Settings)| {
        let (game_seed, game_tick_count, probability_resolution, initial_tile_gold) = hyperparams;
        let initial_tile_resources = BTreeMap::from([(Resource::Gold, initial_tile_gold)]);
        let behaviour_probs = generate_probability_distributions(probability_resolution);
        
        let mut log = String::new();
        let num_of_agents = behaviour_probs.len();
        let reputation_matrix = vec![vec![1f64; num_of_agents]; num_of_agents];
        
        // Actors should be in the same order as behaviour_probs due to the way actors were created.
        let mut tile = Tile::new(vec![], initial_tile_resources.clone(), reputation_matrix);
        for actors_behaviour_probs in behaviour_probs.iter() {
            let actor = Actor::new(BTreeMap::new(), actors_behaviour_probs.clone());
            tile.actors.push(actor);
        }
        
        let hash = hash_hyper_params(&hyperparams);
        
        let plot_file_name = format!("output/{}.gif", hash);
        let mut root = BitMapBackend::gif(plot_file_name, (640, 480), 100).unwrap().into_drawing_area();
        let mut rng = StdRng::seed_from_u64(game_seed as u64);
        
        for tick in 0..game_tick_count {
            log.push_str(&format! ("---------- Game tick {} ----------\n", tick));
            tile.execute_behaviour(&mut rng, &mut log); 
            if (tick % settings.plotting_frame_subselection_factor) == 0 {
                plot_gold_distribution(&tile.actors, &behaviour_probs, &mut root, (tick as u64).try_into().unwrap());
            }
            log.push_str("\n");
        }
        
        if settings.print_game_logs {
            log_general_information(&hyperparams, &mut log);
            log_behaviour_probs(&behaviour_probs, &mut log);
            log_resources(&tile.actors, &mut log);
            log_reputations(&tile.reputations, &mut log);
            
            let file_name = format!("output/{}.txt", hash);    
            write(&file_name, log).unwrap();
        }
    });

    println!("Execution time: {:?}", timer.elapsed());
}
