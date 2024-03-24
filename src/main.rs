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
use ordered_float::OrderedFloat; // Wrapper over f64 to support hashing
use rayon::prelude::*;
use std::cmp::min;
// use rayon::ThreadPoolBuilder;

// This enum exists to support iteration over possibly different types inside variants.
// This is one of three complementary types.
#[derive(Debug, Clone, Hash)]
enum HyperParam { 
    Seed(u64), 
    ProbabilityResolution(usize),
    MintingDifficultyGrowthRate(OrderedFloat<f64>),
}

/// This is one of three complementary types.
#[derive(Debug, Clone, Hash)]
struct HyperParamRanges { 
    seed_values: Vec<HyperParam>,
    probability_resolution_values: Vec<HyperParam>,
    minting_difficulty_growth_rate_values: Vec<HyperParam>,
}

// This type exists to make destructuring of hyperparameter combinations more convenient.
// This is one of three complementary types.
#[derive(Debug, Hash)]
struct CurrentHyperParams {
    seed: u64,
    probability_resolution: usize,
    minting_difficulty_growth_rate: OrderedFloat<f64>,
}

        
#[derive(PartialEq, PartialOrd, Eq, Ord, Debug, Hash, Clone, EnumIter)]
enum Resource {
    Gold,
}

type BehaviourFn = fn(usize, &mut Tile, &mut StdRng, &CurrentHyperParams) -> Result<String, ()>; 

const BEHAVIOURS: [BehaviourFn; 3] = [spend_gold_to_get_rep, mint_gold, collect_tax];
const BEHAVIOUR_NAMES: [&str; 3] = ["spend_gold_to_get_rep", "mint_gold", "collect_tax"];

fn mint_gold(calling_agent_id: usize, tile: &mut Tile, rng: &mut StdRng, hyperparams: &CurrentHyperParams) -> Result<String, ()> {
    let total_gold: usize = tile.agents
    .iter()
    .map(|agent|agent.resources.get(&Resource::Gold).unwrap())
    .sum();

    let probability_to_mint =  1.0/(f64::powi(hyperparams.minting_difficulty_growth_rate.as_f64(), total_gold as i32));

    if rng.gen_bool(probability_to_mint) {
        *tile.agents[calling_agent_id].resources.entry(Resource::Gold).or_insert(0) += 1;

        Ok(format!("Agent {} minted gold. Probability of success is {:.3}.\n", 
        calling_agent_id,
        probability_to_mint))
    } else {
        Ok(format!("Agent {} was not able to mint gold. Probability of success was {:.3}.\n",
        calling_agent_id,
        probability_to_mint))
    }
}

fn collect_tax(calling_agent_id: usize, tile: &mut Tile, rng: &mut StdRng, _hyperparams: &CurrentHyperParams) -> Result<String, ()> {
    let mut total_tax_collected: usize = 0;

    let ids_of_all_other_agents = {
        let mut ids = (0..tile.agents.len()).collect_vec();
        ids.remove(calling_agent_id);
        ids
    };
    
    for target_agent_id in ids_of_all_other_agents {
        let reputation_about_target = tile.reputations[calling_agent_id][target_agent_id];
        let reputation_about_caller = tile.reputations[target_agent_id][calling_agent_id];
        let target_agent_gold = *tile.agents[target_agent_id].resources.get(&Resource::Gold).unwrap();
        let tax: usize = (target_agent_gold as f64 * 0.01).floor() as usize;

        if (reputation_about_caller > reputation_about_target) && rng.gen_bool(0.5) {
            *tile.agents[calling_agent_id].resources.entry(Resource::Gold).or_insert(0) += tax;
            *tile.agents[target_agent_id].resources.entry(Resource::Gold).or_insert(0) -= tax;
            total_tax_collected += tax;
        }
    }

    Ok(String::from(format!("Agent {} collected {} gold from taxes.\n", calling_agent_id, total_tax_collected)))
}

fn spend_gold_to_get_rep(calling_agent_id: usize, tile: &mut Tile, _rng: &mut StdRng, _hyperparams: &CurrentHyperParams) -> Result<String, ()> {
    let initial_agent_gold_amount = *tile.agents[calling_agent_id].resources.get(&Resource::Gold).unwrap_or(&0);
    let gold_required = tile.agents.len() - 1;

    if possble_to_subtract(initial_agent_gold_amount, gold_required) {
        *tile.agents[calling_agent_id].resources.entry(Resource::Gold).or_insert(0) -= gold_required;
        let ids_of_other_agents = {
            let mut agent_ids = (0..tile.agents.len()).collect_vec();
            agent_ids.remove(calling_agent_id);
            agent_ids
        };
        
        for id in ids_of_other_agents {
            *tile.agents[id].resources.entry(Resource::Gold).or_insert(0) += 1;
        }

        tile.reputations.update_reputations_of_others_about_agent(calling_agent_id, |x| *x += 1.0);
        Ok(format!("Agent {} spent {} gold to buy reputation.\n", calling_agent_id, gold_required))
        
    } else {
        Ok(format!("Agent {} has not enough gold to buy reputation ({} vs required {})\n", calling_agent_id, initial_agent_gold_amount, gold_required))
    }
}
fn possble_to_subtract(value: usize, amount_to_substract: usize) -> bool {
    if amount_to_substract <= value {
        true
    } else {
        false
    }
}

type Resources = BTreeMap<Resource, usize>;

#[derive(Debug, Clone)]
struct BehaviourProb {
    behaviour: BehaviourFn,
    probability: f64,
}

#[derive(Debug, Clone)]
struct Agent    {
    behaviours: Vec<BehaviourProb>,
    resources: Resources,
} 

type ReputationMatrix = Vec<Vec<f64>>;

fn log_reputations(m: &ReputationMatrix, log: &mut String) {
    log.push_str("Final reputation matrix: \n");
    let header: String = (0..m[0].len())
        .map(|col_index| format!("{:5}", col_index))
        .collect::<Vec<String>>()
        .join(" ");
    log.push_str(&format!("IDs {}\n", header));

    for (row_index, row) in m.iter().enumerate() {
        let row_str = row.iter()
            .map(|&val| format!("{:5.0}", val))
            .collect::<Vec<String>>()
            .join(" ");
        log.push_str(&format!("{:2}  {}\n", row_index, row_str));
    }
}

fn log_behaviour_probs(behaviour_probs: &Vec<Vec<f64>>, log: &mut String) {
    log.push_str("IDs and behaviours with probabilities: \n");

    for (agent_id, agent_behaviours) in behaviour_probs.iter().enumerate() {
        let row = agent_behaviours.iter().enumerate().map(|(i, behaviour_probability)| {
            let behaviour_name = BEHAVIOUR_NAMES[i];
            format!("{} ({:.0}%)", behaviour_name, behaviour_probability * 100.0)
        }).collect::<Vec<String>>().join(", ");

        log.push_str(&format!("{:2}   [{}]\n", agent_id, row));
    }
    log.push_str("\n");
}

fn log_resources (agents: &Vec<Agent>, log: &mut String) {
    log.push_str("IDs and final resources:\n");
    for (id, agent) in agents.iter().enumerate() {
        log.push_str(&format!("{:2}  {:?}\n", id, &agent.resources));
    }
    log.push_str("\n");
}

#[derive(Debug, Default, Clone)]
struct Tile {
    agents: Vec<Agent>, 
    reputations: ReputationMatrix,
}

impl Agent {
    fn new(initial_resources: Resources, behaviour_probs: Vec<f64>) -> Agent {
        let mut zeroed_resources = Resource::iter().map(|r| (r, 0)).collect::<Resources>();        
        for (resource, amount) in initial_resources {
            zeroed_resources.insert(resource, amount);
        }
        
        let behaviour_probs = zip(BEHAVIOURS.into_iter(), behaviour_probs.into_iter())
            .map(|(behaviour, probability)| BehaviourProb {behaviour, probability})
            .collect();

        Agent {resources: zeroed_resources, behaviours: behaviour_probs}
    }

}

impl Tile {
    fn new(agents: Vec<Agent>, reputations: Vec<Vec<f64>>) -> Tile {
        Tile{agents, reputations}
    }

    fn execute_behaviour(&mut self, rng: &mut StdRng, hyperparams: &CurrentHyperParams) -> String {
        let agent_ids: Vec<usize> = (0..self.agents.len()).collect();
        let mut time_logs = String::new();
        for id in agent_ids {
            let chosen_behaviour: BehaviourFn = {
                let agent = &self.agents[id];
                let probabilities: Vec<f64> = agent.behaviours.iter().map(|b| b.probability).collect();
                let weighted_distribution = WeightedIndex::new(&probabilities).unwrap();
                let chosen_index = weighted_distribution.sample(rng);
                agent.behaviours[chosen_index].behaviour
            };

            // First-come, first-served resource extraction system:
            // If the resource change is possible (thus behaviour is also possible) for the agent we are currently iterating over, the change will occur.
            // Consequently, other agents may fail in attempting to execute exactly the same behavior in the same game tick due to a lack of resources in the Tile.
            let result = chosen_behaviour(id, self, rng, hyperparams);
            
            time_logs.push_str(&result.ok().unwrap())
        }
        time_logs
    }
}

trait UpdateReputations {
    fn update_reputations_of_others_about_agent<F>(&mut self, agent_id: usize, update_fn: F)
    where F: Fn(&mut f64);
}

impl UpdateReputations for ReputationMatrix {
    fn update_reputations_of_others_about_agent<F>(&mut self, agent_id: usize, update_fn: F)
    where F: Fn(&mut f64) {
        for row in self.iter_mut() {
            if let Some(rep) = row.get_mut(agent_id) {
                update_fn(rep);
            }
        }
    }
}

fn generate_probability_distributions(number_of_probability_values: usize) -> Vec<Vec<f64>> {
    match number_of_probability_values {
        0 => {panic!("There should be at least one probability value in range.")},
        1 => {
            let len = BEHAVIOURS.len();
            let probabilities_for_agent = vec![vec![1.0/(len as f64); len]];
            probabilities_for_agent
        }, 
        _ => {
            let mut probabilities_for_all_agents = Vec::new();

            probability_distributions_recursion(
                &mut probabilities_for_all_agents,
                &mut Vec::new(),
                number_of_probability_values - 1,
                BEHAVIOURS.len() - 1,
                number_of_probability_values,
            );
            
            probabilities_for_all_agents
        }
    }
}

fn probability_distributions_recursion(
    probabilities_for_all_agents: &mut Vec<Vec<f64>>,
    probabilities_for_agent: &mut Vec<f64>,
    remaining_probability_steps: usize,
    remaining_recursion_depth: usize,
    number_of_probability_values: usize,
) {
    let probability_step: f64 = 1.0 / (number_of_probability_values - 1) as f64;
    if remaining_recursion_depth == 0 {
        let mut probabilities_for_storage = probabilities_for_agent.clone();
        probabilities_for_storage.push(remaining_probability_steps as f64 * probability_step);
        probabilities_for_all_agents.push(probabilities_for_storage);
    } else {
        for i in 0..=remaining_probability_steps {
            let mut probabilities_for_recursion = probabilities_for_agent.clone();
            probabilities_for_recursion.push(i as f64 * probability_step);
            probability_distributions_recursion(
                probabilities_for_all_agents, 
                &mut probabilities_for_recursion, 
                remaining_probability_steps - i, 
                remaining_recursion_depth - 1,
                number_of_probability_values,
            );
        }
    }
}

fn hash_hyperparams(hyperparams: &CurrentHyperParams) -> u64 {
    let mut hasher = DefaultHasher::new();
    hyperparams.hash(&mut hasher);
    hasher.finish()
}

macro_rules! for_each_hyperparam_combination {
    ($callback:expr) => {{
        let (hps, settings) = read_config();
        vec![
             &hps.seed_values,
             &hps.probability_resolution_values,
             &hps.minting_difficulty_growth_rate_values,
             ]
            .into_iter()
            .multi_cartesian_product()
            .collect::<Vec<_>>()
            .into_par_iter()
            .for_each(|hyperparams| {
                if let [
                        HyperParam::Seed(seed),
                        HyperParam::ProbabilityResolution(probability_resolution),
                        HyperParam::MintingDifficultyGrowthRate(minting_difficulty_growth_rate),
                       ] = &hyperparams[..] {
                        
                        $callback((
                            CurrentHyperParams {
                                seed: *seed,
                                probability_resolution: *probability_resolution,
                                minting_difficulty_growth_rate: *minting_difficulty_growth_rate,
                            }, settings.clone()));
                } else {
                    panic!("Hyperparameters were not parsed correctly.");
                }
            });
        }};
    }
    
fn plot_gold_distribution(
    agents: &Vec<Agent>,
    behavior_probs: &Vec<Vec<f64>>,
    root: &mut DrawingArea<BitMapBackend<'_>, Shift>,
    tick_number: usize,
) {
        
    let log_resources: Vec<f64> = agents.iter()
    .map(|agent| f64::log10(*agent.resources.get(&Resource::Gold).unwrap() as f64))
    .collect();

    let max_log_resource_for_plotting = 4.0;
    let plot_height = 50u32;
    root.fill(&WHITE).unwrap();
    let mut chart = ChartBuilder::on(&root)
        .margin(5)
        .caption("Gold distribution", ("sans-serif", 30))
        .x_label_area_size(40)
        .y_label_area_size(40)
        .build_cartesian_2d(0.0..max_log_resource_for_plotting, 0..plot_height)
        .unwrap();
    chart.configure_mesh().x_desc("log10(Gold)").y_desc("N").draw().unwrap();

    let bucket_count = 100;
    let bucket_width = max_log_resource_for_plotting / bucket_count as f64;
    let mut buckets = vec![0u32; bucket_count];
    for (agent_id, log_resource) in log_resources.iter().enumerate() {
        let bucket_index = min(((log_resource / max_log_resource_for_plotting) * (bucket_count as f64 - 1.0)).floor() as usize, bucket_count-1); 
        // min is used in case value will be too high for the last bucket.
        let color = RGBColor(
            (255.0 * behavior_probs[agent_id][0]) as u8,
            (255.0 * behavior_probs[agent_id][1]) as u8,
            (255.0 * behavior_probs[agent_id][2]) as u8,
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
    full_game_logs: bool,
    tick_count: usize,
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
        plotting_frame_subselection_factor: 1usize, // Default value
        full_game_logs: false, // Default value
        tick_count: 500,  // Default value
    };
    
    for file in &toml_files {
        if file.file_name().unwrap() == "config.toml" {
            println!("File {:?} found", file);
            let toml_map: Value = fs::read_to_string(file).unwrap().parse().unwrap();

            if let Some(Value::Array(settings_map)) = toml_map.get("Settings") {
                for setting in settings_map {

                    let setting1 = "plotting_frame_subselection_factor";
                    let setting2 = "full_game_logs";
                    let setting3 = "tick_count";
                    if let Some(value) = setting.get(setting1) {
                        let extracted_value = value.as_integer().unwrap() as usize;
                        settings.plotting_frame_subselection_factor = extracted_value;
                        println!("{}: {:?}", setting1, extracted_value);
                    }
                    if let Some(value) = setting.get(setting2) {
                        let extracted_value = value.as_bool().unwrap();
                        settings.full_game_logs = extracted_value;
                        println!("{}: {:?}", setting2, extracted_value);
                    }
                    if let Some(value) = setting.get(setting3) {
                        let extracted_value = value.as_integer().unwrap() as usize;
                        settings.tick_count = extracted_value;
                        println!("{}: {:?}", setting3, extracted_value);
                    }
                };
            }
            
            if let Some(Value::Array(hp_map)) = toml_map.get("Hyperparameters") {
                for hp in hp_map {
                    let game_seed_values = read_usize_vec_entry(hp, "game_seed_values")
                        .expect("File config.toml should contain game_seed_values entry in [[Hyperparameters]] with at least one usize value in a list.")
                        .into_iter().map(|seed| HyperParam::Seed(seed as u64)).collect();
                    let probability_resolution_values = read_usize_vec_entry(hp, "probability_resolution_values")
                        .expect("File config.toml should contain probability_resolution_values entry in [[Hyperparameters]] with at least one usize value in a list.")
                        .into_iter().map(HyperParam::ProbabilityResolution).collect();
                    let minting_difficulty_growth_rate_values = read_float_vec_entry(hp, "minting_difficulty_growth_rate_values")
                        .expect("File config.toml should contain minting_difficulty_growth_rate_values entry in [[Hyperparameters]] with at least one float value in a list.")
                        .into_iter().map(HyperParam::MintingDifficultyGrowthRate).collect();
                    
                    let hp_ranges = HyperParamRanges {
                        seed_values: game_seed_values,
                        probability_resolution_values,
                        minting_difficulty_growth_rate_values: minting_difficulty_growth_rate_values,
                    };

                    return  (hp_ranges, settings)
                }
            } else {panic!("[[Hyperparameters]] section was not found in config.toml") }
        }
    } panic!("config.toml was not found") 
}

fn read_usize_vec_entry(hyperparameter: &Value, key: &str) -> Option<Vec<usize>> {
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

fn read_float_vec_entry(hyperparameter: &Value, key: &str) -> Option<Vec<OrderedFloat<f64>>> {
    if let Some(Value::Array(values)) = hyperparameter.get(key) {
        let extracted_values = values
        .iter().filter_map(|v| {
            if let Value::Float(value) = v { Some(*value) } else { None }
        })
        .map(|f|OrderedFloat::from(f))
        .collect();

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
    for_each_hyperparam_combination!(|(hyperparams, settings): (CurrentHyperParams, Settings)| {
        let behaviour_probs = generate_probability_distributions(hyperparams.probability_resolution);
        
        let num_of_agents = behaviour_probs.len();
        let reputation_matrix = vec![vec![1f64; num_of_agents]; num_of_agents];
        
        // Agents should be in the same order as behaviour_probs due to the way agents were created.
        let mut tile = Tile::new(vec![], reputation_matrix);
        for agent_behaviour_probs in behaviour_probs.iter() {
            let agent = Agent::new(BTreeMap::new(), agent_behaviour_probs.clone());
            tile.agents.push(agent);
        }
        
        let hash = hash_hyperparams(&hyperparams);
        
        let plot_file_pathname = format!("output/{}.gif", hash);
        let mut root = BitMapBackend::gif(plot_file_pathname, (640, 480), 100).unwrap().into_drawing_area();

        let mut rng = StdRng::seed_from_u64(hyperparams.seed as u64);
        let mut optional_log = String::new();
        
        for tick in 0..settings.tick_count {
            let optional_log_fragment = tile.execute_behaviour(&mut rng, &hyperparams); 
            
            if settings.full_game_logs {
                optional_log.push_str(&format! ("---------- Game tick {} ----------\n", tick));
                optional_log.push_str(&optional_log_fragment);
                optional_log.push_str("\n");
            }
            
            if (tick % settings.plotting_frame_subselection_factor) == 0 {
                plot_gold_distribution(&tile.agents, &behaviour_probs, &mut root, (tick as u64).try_into().unwrap());
            }
        }
        
        let mut summary_log = String::new();
        summary_log.push_str(&format!("{:#?}\n{:#?}\n", hyperparams, settings));
        log_behaviour_probs(&behaviour_probs, &mut summary_log);
        log_resources(&tile.agents, &mut summary_log);
        log_reputations(&tile.reputations, &mut summary_log);
        
        let log_file_pathname = format!("output/{}.txt", hash);
        write(&log_file_pathname, summary_log + &optional_log).unwrap();
    });

    println!("Execution time: {:.3} s", timer.elapsed().as_secs_f64());
}
