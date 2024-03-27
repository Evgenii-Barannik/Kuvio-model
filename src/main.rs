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

type AgentID = usize;

#[derive(Eq, Ord, PartialEq, PartialOrd)]
enum Resource {
    Coins,
}

type Resources = BTreeMap<Resource, usize>;
type Action = fn(Vec<Resources>) -> Vec<Resources>;
type ActionSetModification = fn(Vec<&Action>) -> Vec<&Action>;
type Behaviour = fn(Vec<&Action>, DecisionMakingData, &mut StdRng) -> Action;
struct DecisionMakingData {} // TODO

struct Agent {
    resources: Resources,
    actions: Vec<Action>,
    behaviour: Behaviour,
}


struct Game  {
    roles: BTreeMap<AnyRole, Vec<AgentID>>,
    action_set_modifications: BTreeMap<AnyRole, ActionSetModification>,
}
type GameProvider = fn() -> Game;

type ReputationMatrix = Vec<Vec<f64>>;

struct Tile {
    agents: Vec<Agent>,
    resources: Resources,
    reputations: ReputationMatrix,
}

///

enum AnyRole {
    KingdomRole,
    LoversRole,
}

enum KingdomRole {
    King,
    Guard,
    Peasant,
}

enum LoversRole {
    LoverOne,
    LoverTwo,
}


const PASS: Action = |mut resources_arr: Vec<Resources>| -> Vec<Resources> {
    resources_arr
};

const MINT: Action = |mut resources_arr: Vec<Resources>| -> Vec<Resources> {
    for res in &mut resources_arr {
        *res.entry(Resource::Coins).or_insert(0) += 1;
    }
    resources_arr
};
const UNIVERSAL_ACTIONS: [Action; 2] = [PASS, MINT];

const BEHAVIOUR1


fn main() {
    let timer = Instant::now();
    fs::create_dir_all("output").unwrap();

    // // rayon::ThreadPoolBuilder::new().num_threads(1).build_global().unwrap();
    // for_each_hyperparam_combination!(|(hyperparams, settings): (CurrentHyperParams, Settings)| {
    //     let behaviour_probs = generate_probability_distributions(hyperparams.probability_resolution);
        
    //     let num_of_agents = behaviour_probs.len();
    //     // let reputation_matrix = vec![vec![1f64; num_of_agents]; num_of_agents];
        
    //     // Agents should be in the same order as behaviour_probs due to the way agents were created.
    //     // let mut tile = Tile::new(vec![], reputation_matrix);
    //     for agent_behaviour_probs in behaviour_probs.iter() {
    //         let agent = Agent::new(BTreeMap::new(), agent_behaviour_probs.clone());
    //         tile.agents.push(agent);
    //     }
        
    //     let hash = hash_hyperparams(&hyperparams);
        
    //     let plot_file_pathname = format!("output/{}.gif", hash);
    //     let mut root = BitMapBackend::gif(plot_file_pathname, (640, 480), 100).unwrap().into_drawing_area();

    //     let mut rng = StdRng::seed_from_u64(hyperparams.seed as u64);
    //     let mut optional_log = String::new();
        
    //     for tick in 0..settings.tick_count {
    //         let optional_log_fragment = tile.execute_behaviour(&mut rng, &hyperparams); 
            
    //         if settings.full_game_logs {
    //             optional_log.push_str(&format! ("---------- Game tick {} ----------\n", tick));
    //             optional_log.push_str(&optional_log_fragment);
    //             optional_log.push_str("\n");
    //         }
            
    //         if (tick % settings.plotting_frame_subselection_factor) == 0 {
    //             plot_gold_distribution(&tile.agents, &behaviour_probs, &mut root, (tick as u64).try_into().unwrap());
    //         }
    //     }
        
    //     let mut summary_log = String::new();
    //     summary_log.push_str(&format!("{:#?}\n{:#?}\n", hyperparams, settings));
    //     log_behaviour_probs(&behaviour_probs, &mut summary_log);
    //     log_resources(&tile.agents, &mut summary_log);
    //     log_reputations(&tile.reputations, &mut summary_log);
        
    //     let log_file_pathname = format!("output/{}.txt", hash);
    //     write(&log_file_pathname, summary_log + &optional_log).unwrap();
    // });

    println!("Execution time: {:.3} s", timer.elapsed().as_secs_f64());
}
