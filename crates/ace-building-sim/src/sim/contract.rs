use crate::model::*;
use crate::validate::known_type;

pub(crate) fn is_g36_graph(graph: &Block) -> bool {
    graph.name == "FourPipeG36Controls"
        || graph
            .blocks
            .iter()
            .any(|block| block.block_type == "ZoneGroupModeController")
}

fn cdl_output_has_block_driver(block: &Block, port: &str) -> bool {
    if !block.outputs.iter().any(|output| output.name == port) {
        return false;
    }
    if block.blocks.is_empty() {
        return block.category.as_deref() != Some("composite")
            && block.block_type.rsplit('.').next() != Some("VAVBoxController")
            && known_type(&block.block_type);
    }
    block.connections.iter().any(|edge| {
        let (target, target_port) = edge.target();
        if target != port || !target_port.is_empty() {
            return false;
        }
        let (source, source_port) = edge.source();
        block
            .blocks
            .iter()
            .any(|child| child.name == source && cdl_output_has_block_driver(child, &source_port))
    })
}

pub(crate) fn undriven_building_outputs(graph: &Block) -> Vec<String> {
    if !graph
        .blocks
        .iter()
        .any(|block| block.block_type == "AHUController")
    {
        return Vec::new();
    }
    graph
        .outputs
        .iter()
        .filter(|output| {
            !graph.connections.iter().any(|edge| {
                let (target, target_port) = edge.target();
                if target != output.name || !target_port.is_empty() {
                    return false;
                }
                let (source, source_port) = edge.source();
                graph.blocks.iter().any(|block| {
                    block.name == source && cdl_output_has_block_driver(block, &source_port)
                })
            })
        })
        .map(|output| {
            format!(
                "Building output {} must have a CDL block driver",
                output.name
            )
        })
        .collect()
}

pub(crate) fn g36_control_contract(graph: &Block) -> Vec<String> {
    if !is_g36_graph(graph) {
        return Vec::new();
    }
    let mut errors = Vec::new();
    let names = super::points::output_names(true);
    for name in names {
        let declared = graph.outputs.iter().any(|port| port.name == name);
        let driven = graph.connections.iter().any(|edge| {
            if edge.target().0 != name {
                return false;
            }
            let (source, port) = edge.source();
            graph
                .blocks
                .iter()
                .any(|block| block.name == source && cdl_output_has_block_driver(block, &port))
        });
        if !declared || !driven {
            errors.push(format!(
                "G36 simulation requires a CDL-driven {name} output"
            ));
        }
    }
    errors
}

pub(crate) fn conventional_control_contract(graph: &Block) -> Vec<String> {
    if is_g36_graph(graph)
        || !graph
            .blocks
            .iter()
            .any(|block| block.block_type == "AHUController")
    {
        return Vec::new();
    }
    let mut errors = Vec::new();
    let names = super::points::output_names(false);
    for name in names {
        let driven = graph.connections.iter().any(|edge| {
            if edge.target().0 != name {
                return false;
            }
            let (source, port) = edge.source();
            graph
                .blocks
                .iter()
                .any(|block| block.name == source && cdl_output_has_block_driver(block, &port))
        });
        if !graph.outputs.iter().any(|output| output.name == name) || !driven {
            errors.push(format!(
                "Building simulation requires a CDL-driven {name} output"
            ));
        }
    }
    errors
}
