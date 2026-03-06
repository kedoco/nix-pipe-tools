use crate::db::Database;
use petgraph::graph::DiGraph;
use std::collections::HashMap;

pub fn build_graph(
    db: &Database,
    file: &str,
) -> anyhow::Result<(DiGraph<String, String>, HashMap<String, petgraph::graph::NodeIndex>)> {
    let mut graph = DiGraph::new();
    let mut nodes: HashMap<String, petgraph::graph::NodeIndex> = HashMap::new();

    let events = db.all_events_for_file(file)?;

    for (cmd, file_events) in &events {
        let mut reads = Vec::new();
        let mut writes = Vec::new();

        for ev in file_events {
            match ev.event_type.as_str() {
                "read" => reads.push(ev.path.clone()),
                "write" | "create" => writes.push(ev.path.clone()),
                _ => {}
            }
        }

        // Add nodes for all files
        for path in reads.iter().chain(writes.iter()) {
            if !nodes.contains_key(path) {
                let idx = graph.add_node(path.clone());
                nodes.insert(path.clone(), idx);
            }
        }

        // Add edges: each read -> each write, labeled with command
        for r in &reads {
            for w in &writes {
                if r != w {
                    let ri = nodes[r];
                    let wi = nodes[w];
                    graph.add_edge(ri, wi, cmd.command.clone());
                }
            }
        }
    }

    Ok((graph, nodes))
}

pub fn to_dot(graph: &DiGraph<String, String>) -> String {
    let mut out = String::from("digraph {\n");
    for edge in graph.edge_indices() {
        let (src, dst) = graph.edge_endpoints(edge).unwrap();
        let label = &graph[edge];
        let src_name = &graph[src];
        let dst_name = &graph[dst];
        out.push_str(&format!(
            "    \"{}\" -> \"{}\" [label=\"{}\"];\n",
            src_name, dst_name, label
        ));
    }
    out.push_str("}\n");
    out
}

pub fn to_mermaid(graph: &DiGraph<String, String>) -> String {
    let mut out = String::from("graph TD;\n");
    for edge in graph.edge_indices() {
        let (src, dst) = graph.edge_endpoints(edge).unwrap();
        let label = &graph[edge];
        let src_name = &graph[src];
        let dst_name = &graph[dst];
        // Mermaid node IDs can't have special chars, use sanitized versions
        let src_id = sanitize_mermaid_id(src_name);
        let dst_id = sanitize_mermaid_id(dst_name);
        out.push_str(&format!(
            "    {}[\"{}\"] -->|{}| {}[\"{}\"];\n",
            src_id, src_name, label, dst_id, dst_name
        ));
    }
    out
}

fn sanitize_mermaid_id(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect()
}
