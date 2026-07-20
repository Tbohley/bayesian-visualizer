use super::*;


#[derive(Clone, Copy, PartialEq, Eq)]
enum VisitState {
    Visiting,
    Visited,
}


impl GraphIR{
    pub fn check_cycles(&self) -> Result<(), Vec<u32>> {
        fn params(node: &NodeIR) -> &[ParamIR] {
            match node {
                NodeIR::Random { params, .. } => params,
                NodeIR::Compute { params, .. } => params,
                NodeIR::Scalar { .. } => &[],
            }
        }

        fn visit(
            node_id: u32,
            graph: &GraphIR,
            states: &mut HashMap<u32, VisitState>,
            stack: &mut Vec<u32>,
        ) -> Result<(), Vec<u32>> {
            if states.get(&node_id) == Some(&VisitState::Visiting) {
                let cycle_start = stack
                    .iter()
                    .position(|id| *id == node_id)
                    .expect("visiting node should be in DFS stack");

                return Err(stack[cycle_start..].to_vec());
            }

            if states.get(&node_id) == Some(&VisitState::Visited) {
                return Ok(());
            }

            states.insert(node_id, VisitState::Visiting);
            stack.push(node_id);

            if let Some(node) = graph.nodes.get(&node_id) {
                for param in params(node) {
                    visit(param.from_node, graph, states, stack)?;
                }
            }

            stack.pop();
            states.insert(node_id, VisitState::Visited);

            Ok(())
        }

        let mut states = HashMap::new();
        let mut stack = Vec::new();

        for &node_id in self.nodes.keys() {
            visit(node_id, self, &mut states, &mut stack)?;
        }

        Ok(())
    }
}