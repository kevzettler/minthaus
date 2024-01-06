use std::collections::HashMap;
extern crate psd;

use base64::{engine::general_purpose, Engine as _};
use image::{ImageBuffer, Rgba};
use psd::{Psd, PsdGroup, PsdLayer};

#[derive(Debug)]
enum PsdNode<'a> {
    Group(u32, &'a PsdGroup),
    Layer(u32, &'a PsdLayer),
}

fn main() {
    // Example: Encode a token
    let token = "FFFF00000300030003"; // token corresponding to layers
    let encoded_token = general_purpose::URL_SAFE_NO_PAD.encode(token);
    println!("Encoded Token: {}", encoded_token);

    let input_token = encoded_token;
    let decoded_token = match general_purpose::URL_SAFE_NO_PAD.decode(input_token.as_bytes()) {
        Ok(decoded) => String::from_utf8_lossy(&decoded).to_string(), // Convert to owned String
        Err(e) => {
            println!("Failed to decode token: {}", e);
            return;
        }
    };

    let group_visibility = decode_hex_to_visibility(&decoded_token);

    let psd_bytes = include_bytes!("../kz-wingrivals-chars-final.psd");
    let psd = Psd::from_bytes(psd_bytes).unwrap();
    let mut top_level_groups = get_top_level_groups(&psd);
    // Sort the groups by their ID in ascending order
    top_level_groups.sort_by_key(|(id, _)| *id);

    let mut layer_indices_to_flatten = Vec::new();
    for (index, (group_id, _group)) in top_level_groups.iter().enumerate() {
        if let Some(visibility) = group_visibility.get(index) {
            if *visibility == 255 {
                // visibility 255, push all layers
                if let Some(sub_layers) = psd.get_group_sub_layers(group_id) {
                    for layer in sub_layers {
                        let identifier = get_layer_identifier(&psd, layer);
                        layer_indices_to_flatten.push(identifier);
                    }
                }
            } else if *visibility != 0 {
                // Only push the visible layers
                if let Some(sub_layers) = psd.get_group_sub_layers(group_id) {
                    for layer in sub_layers {
                        let possible_sub_index = layer.parent_id().unwrap_or(0);
                        let identifier = get_layer_identifier(&psd, layer);
                        if possible_sub_index == *group_id + u32::from(*visibility) {
                            layer_indices_to_flatten.push(identifier);
                        }
                    }
                }
            }
        }
    }

    // iterate over the background layers
    // iterate over the visibile layers.
    // if theres a match addd the background layer to the list
    let background_layers = get_background_layers(&psd);
    for (_, bg_identifier) in background_layers.keys().enumerate() {
        for (_, identifier) in layer_indices_to_flatten.iter().enumerate() {
            // TODO need to do appropriate string comparison here
            if let Some(first_part) = identifier.splitn(2, '-').next() {
                if bg_identifier.contains(first_part) {
                    if let Some(background_node) = background_layers.get(bg_identifier) {
                        match background_node {
                            PsdNode::Group(group_id, _group) => {
                                if let Some(sub_layers) = psd.get_group_sub_layers(group_id) {
                                    for layer in sub_layers {
                                        let identifier = get_layer_identifier(&psd, layer);
                                        layer_indices_to_flatten.push(identifier);
                                    }
                                }
                            }
                            PsdNode::Layer(_layer_id, _layer) => {
                                layer_indices_to_flatten.push(bg_identifier.clone());
                            }
                        }
                    }
                    break;
                }
            }
        }
    }
    flatten_layers_and_output_png(layer_indices_to_flatten, &psd);
}

fn get_top_level_groups(psd: &Psd) -> Vec<(u32, &PsdGroup)> {
    psd.groups()
        .iter()
        .filter_map(|(&id, group)| {
            if group.parent_id().is_none() && !group.name().starts_with("_bg") {
                Some((id, group))
            } else {
                None
            }
        })
        .collect()
}

fn get_background_layers(psd: &Psd) -> HashMap<String, PsdNode> {
    let mut elements = HashMap::new();

    // Add groups prefixed with "_bg"
    for (&id, group) in psd.groups() {
        if group.parent_id().is_none() && group.name().starts_with("_bg") {
            elements.insert(group.name().to_string(), PsdNode::Group(id, group));
        }
    }

    // Add layers prefixed with "_bg"
    for (id, layer) in psd.layers().iter().enumerate() {
        if layer.name().starts_with("_bg") {
            elements.insert(layer.name().to_string(), PsdNode::Layer(id as u32, layer));
        }
    }

    elements
}

fn decode_hex_to_visibility(hex_str: &str) -> Vec<u8> {
    (0..hex_str.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex_str[i..i + 2], 16).unwrap_or(0))
        .collect()
}

fn flatten_layers_and_output_png(layer_identifiers: Vec<String>, psd: &Psd) {
    // Flatten the layers
    let flattened_image = psd
        .flatten_layers_rgba(&|(_idx, layer)| {
            let identifier = get_layer_identifier(psd, layer);
            layer_identifiers.contains(&identifier)
        })
        .unwrap();

    // Convert the flattened image to an ImageBuffer and save as PNG
    let image_buffer =
        ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(psd.width(), psd.height(), flattened_image)
            .unwrap();
    image_buffer.save("./test-output.png").unwrap();
}

fn get_layer_identifier(psd: &Psd, layer: &PsdLayer) -> String {
    let all_groups = psd.groups();
    let possible_sub_index = layer.parent_id().unwrap_or(0);
    let parent_group_name = match all_groups.get(&possible_sub_index) {
        Some(psd_group) => psd_group.name(),
        None => "unkown",
    };
    let identifier = format!(
        "{}-{}-{}",
        parent_group_name,
        layer.parent_id().unwrap_or(0),
        layer.name()
    );
    identifier
}
