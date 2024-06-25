use std::fs;
use std::vec;
use std::iter::IntoIterator;
use plotters::{coord::Shift, prelude::*};
use toml::Value;
use std::path::PathBuf;
use walkdir::WalkDir;
use std::cmp::min;

use super::{AnyResource, Tile};

#[derive(Debug, Clone)]
pub struct Configs { 
    pub plot_graph: bool, 
    pub plotting_frame_subselection_factor: usize, 
    pub tick_count: usize, 
    pub agent_count: usize,
    pub seed: usize,
}

fn try_to_read_integer(entry: &Value, searched_var: &str) -> usize {
    let value = entry.get(searched_var).expect(&format!("{} variable not found", searched_var));
    let extracted_value = value.as_integer().unwrap();
    extracted_value as usize
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
                for entry in config_map {

                    let c1 = "plot_graph";
                    let c2 = "plotting_frame_subselection_factor";
                    let c3 = "tick_count";
                    let c4 = "agent_count";
                    let c5 = "seed";

                    let plot_graph = {
                        let value = entry.get(c1).expect(&format!("{} config not found", c1));
                        let extracted_value = value.as_bool().unwrap();
                        extracted_value
                    };
                    
                    let plotting_frame_subselection_factor = try_to_read_integer(entry, c2);
                    let tick_count = try_to_read_integer(entry, c3);
                    let agent_count = try_to_read_integer(entry, c4);
                    let seed = try_to_read_integer(entry, c5); 

                    let configs = Configs { 
                        plot_graph,
                        plotting_frame_subselection_factor,
                        tick_count,
                        agent_count,
                        seed,
                    };
                    
                    println!("{:#?}\n", configs);
                    return configs
                };
            }
        }
    }
    panic!("config.toml was not read") 
}

pub fn plot_resource_distribution(tile: &Tile, root: &mut DrawingArea<BitMapBackend<'_>, Shift>, tick_number: usize) {
    let max_log_resource_for_plotting = 4.0;
    let plot_height = 10u32;
    let bucket_count = 100;
    let bucket_width = max_log_resource_for_plotting / bucket_count as f64;
    let colormap = VulcanoHSL {};
    
    let text_size = 15;
    let tick_info = &format!("Tick: {}", tick_number);
    
    root.fill(&WHITE).unwrap();
    let mut chart = ChartBuilder::on(&root)
        .margin(5)
        .caption("Coin distribution", ("sans-serif", 30))
        .x_label_area_size(40)
        .y_label_area_size(40)
        .build_cartesian_2d(0.0..max_log_resource_for_plotting, 0..plot_height)
        .unwrap();
    chart.configure_mesh().x_desc("log10(Coin)").y_desc("N").draw().unwrap();

    let mut buckets = vec![0u32; bucket_count];
    let mut rectangles_to_draw = vec![];
    let mut ids_to_draw = vec![];

    let agent_resources = tile.agents.iter()
        .map(|agent| *agent.resources.get(&AnyResource::Coins).unwrap() as f64)
        .collect::<Vec<f64>>();

    let agents_plus_tile_resources = {   // Tile resource value is the last element in vec
        let mut resources = agent_resources.clone();
        resources.push(*tile.resources.get(&AnyResource::Coins).unwrap() as f64);  
        resources
    };

    for (agent_id, resource) in agents_plus_tile_resources.iter().enumerate() {
        let log_resource = f64::log10(*resource);
        let bucket_index = min(((log_resource / max_log_resource_for_plotting) * (bucket_count as f64 - 1.0)).floor() as usize, bucket_count-1); 
        let relative_position = agent_id as f32 / agent_resources.len() as f32;
        let color = colormap.get_color(relative_position);

        let bar_left = bucket_index as f64 * bucket_width;
        let bar_right = bar_left + bucket_width;
        let bar_bottom = buckets[bucket_index];
        let bar_top = bar_bottom + 1;

        if buckets[bucket_index] < plot_height { // To Draw only if will be visible.
            rectangles_to_draw.push(
                Rectangle::new(
                    [(bar_left, bar_bottom), (bar_right, bar_top)],
                    color.filled())
            );
    
            let padded_id = if !(agent_id == agents_plus_tile_resources.len() - 1) {
                format!("{:<3}", agent_id) // For Agents
            } else {
                "T  ".to_string() // For Tile
            };
                
            ids_to_draw.push(
                EmptyElement::<(f64, u32), BitMapBackend<'_>>::at(((bar_left+bar_right)/2.0, bar_top)) 
                + Text::new(padded_id[0..1].to_string(), (-3, 0), ("sans-serif", text_size-2).into_font())
                + Text::new(padded_id[1..2].to_string(), (-3, 10), ("sans-serif", text_size-2).into_font())
                + Text::new(padded_id[2..3].to_string(), (-3, 20), ("sans-serif", text_size-2).into_font())
            );

            buckets[bucket_index]+= 1;
        }
    }

    root.draw(&Text::new(
        "Numbers in rectangles are Agent IDs, written from top to bottom.",
        (160, 50),
        ("sans-serif", text_size + 5).into_font(),
    )).unwrap();

    root.draw(&Text::new(
        tick_info.as_str(),
        (550, 20),
        ("sans-serif", text_size).into_font(),
    )).unwrap();

    chart.draw_series(rectangles_to_draw).unwrap();
    chart.draw_series(ids_to_draw).unwrap();
    root.present().unwrap();
}
