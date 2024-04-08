use std::borrow::BorrowMut;
use std::fs;
use std::time::Instant;
use std::vec;
use std::collections::{BTreeMap};
use std::hash::{Hash, Hasher};
use std::fs::write;
use std::vec::Drain;
use itertools::Itertools;
use std::iter::IntoIterator;
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
use std::cmp::min;
use rand::distributions::Uniform; 
use rayon::prelude::*;
// use rayon::ThreadPoolBuilder;
use std::iter::zip;
use rand::prelude::SliceRandom;
// use strum::IntoEnumIterator;

use super::{Agent, AnyResource, ReputationMatrix};
#[derive(Debug, Clone)]
pub struct Configs { 
    pub plot_graph: bool, 
    pub plotting_frame_subselection_factor: usize, 
    pub tick_count: usize, 
    pub agent_count: usize,
    pub seed: usize,
}

pub fn read_configs() -> Configs {
    let toml_files: Vec<PathBuf> = WalkDir::new(".")
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| 
            entry.file_type().is_file() &&
            entry.file_name().to_string_lossy().ends_with(".toml"))
        .map(|entry| entry.into_path() )
        .collect();
    
    for file in &toml_files {
        if file.file_name().unwrap() == "config.toml" {
            println!("File {:?} found", file);
            let toml_map: Value = fs::read_to_string(file).unwrap().parse().unwrap();

            if let Some(Value::Array(config_map)) = toml_map.get("Configs") {
                for config in config_map {

                    let c1 = "plot_graph";
                    let c2 = "plotting_frame_subselection_factor";
                    let c3 = "tick_count";
                    let c4 = "agent_count";
                    let c5 = "seed";

                    let plot_graph = {
                        let value = config.get(c1).expect(&format!("{} config not found", c1));
                        let extracted_value = value.as_bool().unwrap();
                        println!("{}: {:?}", c1, extracted_value);
                        extracted_value
                    };
                    
                    let plotting_frame_subselection_factor = {
                        let value = config.get(c2).expect(&format!("{} config not found", c2));
                        let extracted_value = value.as_integer().unwrap();
                        println!("{}: {:?}", c2, extracted_value);
                        extracted_value as usize
                    };
                    
                    let tick_count = {
                        let value = config.get(c3).expect(&format!("{} config not found", c3));
                        let extracted_value = value.as_integer().unwrap();
                        println!("{}: {:?}", c3, extracted_value);
                        extracted_value as usize
                    };

                    let agent_count = {
                        let value = config.get(c4).expect(&format!("{} config not found", c4));
                        let extracted_value = value.as_integer().unwrap();
                        println!("{}: {:?}", c4, extracted_value); 
                        extracted_value as usize
                    };

                    let seed = {
                        let value = config.get(c5).expect(&format!("{} config not found", c5));
                        let extracted_value = value.as_integer().unwrap();
                        println!("{}: {:?}", c5, extracted_value);
                        extracted_value as usize
                    };

                    let configs = Configs { 
                        plot_graph,
                        plotting_frame_subselection_factor,
                        tick_count,
                        agent_count,
                        seed,
                    };
                
                    return configs
                };
            }
        }
    }
    panic!("config.toml was not read") 
}


pub fn log_resources (agents: &Vec<Agent>, log: &mut String) {
    log.push_str("Agent IDs and final resources:\n");
    for agent in agents.iter() {
        log.push_str(&format!("{:2}  {:?}\n", agent.id, &agent.resources));
    }
    log.push_str("\n");
}

pub fn log_reputations(m: &ReputationMatrix, log: &mut String) {
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

// fn plot_gold_distribution(
//     agents: &Vec<Agent>,
//     behavior_probs: &Vec<Vec<f64>>,
//     root: &mut DrawingArea<BitMapBackend<'_>, Shift>,
//     tick_number: usize,
// ) {
        
//     let log_resources: Vec<f64> = agents.iter()
//     .map(|agent| f64::log10(*agent.resources.get(&Resource::Gold).unwrap() as f64))
//     .collect();

//     let max_log_resource_for_plotting = 4.0;
//     let plot_height = 50u32;
//     root.fill(&WHITE).unwrap();
//     let mut chart = ChartBuilder::on(&root)
//         .margin(5)
//         .caption("Gold distribution", ("sans-serif", 30))
//         .x_label_area_size(40)
//         .y_label_area_size(40)
//         .build_cartesian_2d(0.0..max_log_resource_for_plotting, 0..plot_height)
//         .unwrap();
//     chart.configure_mesh().x_desc("log10(Gold)").y_desc("N").draw().unwrap();

//     let bucket_count = 100;
//     let bucket_width = max_log_resource_for_plotting / bucket_count as f64;
//     let mut buckets = vec![0u32; bucket_count];
//     for (agent_id, log_resource) in log_resources.iter().enumerate() {
//         let bucket_index = min(((log_resource / max_log_resource_for_plotting) * (bucket_count as f64 - 1.0)).floor() as usize, bucket_count-1); 
//         // min is used in case value will be too high for the last bucket.
//         let color = RGBColor(
//             (255.0 * behavior_probs[agent_id][0]) as u8,
//             (255.0 * behavior_probs[agent_id][1]) as u8,
//             (255.0 * behavior_probs[agent_id][2]) as u8,
//         );

//         let bar_left = bucket_index as f64 * bucket_width;
//         let bar_right = bar_left + bucket_width;
//         let bar_bottom = buckets[bucket_index];
//         let bar_top = bar_bottom + 1;

//         chart.draw_series(std::iter::once(Rectangle::new(
//             [(bar_left, bar_bottom), (bar_right, bar_top)],
//             color.filled(),
//         ))).unwrap();

//         buckets[bucket_index]+= 1;
//     }

//     let (legend_x, legend_y) = (55, 55);
//     let legend_size = 15;
//     let text_gap = 5;
//     let text_size = 15;

//     let tick_info = format!("Tick: {}", tick_number);

//     let legend_entries = vec![
//         (BEHAVIOUR_NAMES[0], RED),
//         (BEHAVIOUR_NAMES[1], GREEN),
//         (BEHAVIOUR_NAMES[2], BLUE),
//         (&tick_info, WHITE)
//     ];

//     for (i, (label, color)) in legend_entries.iter().enumerate() {
//         let y_position = legend_y + i as i32 * (legend_size + text_gap + text_size);

//         root.draw(&Rectangle::new(
//             [(legend_x, y_position), (legend_x + legend_size, y_position + legend_size)],
//             color.filled(),
//         )).unwrap();

//         root.draw(&Text::new(
//             *label,
//             (legend_x + legend_size + text_gap, y_position + (legend_size / 2)),
//             ("sans-serif", text_size).into_font(),
//         )).unwrap();
//     }

//     root.present().unwrap();
// }
